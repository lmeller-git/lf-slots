#![allow(dead_code)]

use alloc::{collections::VecDeque, vec::Vec};

use crate::{
    slot_alloc::SlotStorage,
    sync::{
        Arc,
        Condvar,
        Mutex,
        atomic::{AtomicUsize, Ordering},
        thread,
    },
};

#[cfg(any(miri, loom, shuttle))]
pub(crate) const THREADS: usize = 2;
#[cfg(not(any(miri, loom, shuttle)))]
pub(crate) const THREADS: usize = 4;

#[cfg(any(miri, loom, shuttle))]
pub(crate) const COUNT: usize = 50;
#[cfg(not(any(miri, loom, shuttle)))]
pub(crate) const COUNT: usize = 25_000;

fn backoff() {
    #[cfg(any(loom, shuttle))]
    thread::yield_now();

    #[cfg(not(any(loom, shuttle)))]
    crate::sync::hint::spin_loop();
}

pub(crate) fn smoke<S>(storage: S)
where
    S: SlotStorage,
{
    let i0 = storage.pull().unwrap();
    let i1 = storage.pull().unwrap();
    assert_ne!(i0, i1);

    assert!(storage.put(i0));
    let i2 = storage.pull().unwrap();
    assert_eq!(i0, i2);
}

pub(crate) fn smoke_long<S>(storage: S)
where
    S: SlotStorage,
{
    assert_eq!(storage.capacity(), 10);

    assert!(storage.is_empty());
    for _ in 0..10 {
        assert!(storage.pull().is_some());
    }
    assert!(storage.is_full());
    assert_eq!(storage.len(), 10);

    assert!(storage.put(5));
    assert!(!storage.is_full());

    assert_eq!(storage.len(), 9);

    assert_eq!(storage.pull(), Some(5));

    for i in 0..10 {
        assert!(storage.put(i));
    }

    assert!(storage.is_empty());
    assert_eq!(storage.len(), 0);

    assert_eq!(storage.pull(), Some(0));
}

pub(crate) fn len_empty_full<S>(storage: S)
where
    S: SlotStorage,
{
    assert_eq!(storage.capacity(), 2);
    assert_eq!(storage.len(), 0);
    assert!(storage.is_empty());
    assert!(!storage.is_full());

    let i0 = storage.pull().unwrap();
    assert_eq!(storage.len(), 1);
    assert!(!storage.is_empty());
    assert!(!storage.is_full());

    let i1 = storage.pull().unwrap();
    assert_eq!(storage.len(), 2);
    assert!(!storage.is_empty());
    assert!(storage.is_full());

    assert!(storage.put(i0));
    assert_eq!(storage.len(), 1);
    assert!(!storage.is_empty());
    assert!(!storage.is_full());

    assert!(storage.put(i1));
    assert_eq!(storage.len(), 0);
    assert!(storage.is_empty());
    assert!(!storage.is_full());
}

pub(crate) struct BlockingMpscChannel {
    queue: Mutex<VecDeque<usize>>,
    cond: Condvar,
}

impl BlockingMpscChannel {
    pub(crate) fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            cond: Condvar::new(),
        }
    }

    pub(crate) fn send(&self, val: usize) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(val);
        self.cond.notify_one();
    }

    pub(crate) fn recv(&self) -> usize {
        let mut q = self.queue.lock().unwrap();
        while q.is_empty() {
            q = self.cond.wait(q).unwrap();
        }
        q.pop_front().unwrap()
    }
}

pub(crate) fn spsc<S>(storage: S)
where
    S: SlotStorage + Send + Sync + 'static,
{
    let storage = Arc::new(storage);
    let channel = Arc::new(BlockingMpscChannel::new());

    let s_clone = storage.clone();
    let c_clone = channel.clone();
    let producer = thread::spawn(move || {
        for _ in 0..COUNT {
            let idx = loop {
                if let Some(idx) = s_clone.pull() {
                    break idx;
                }
                backoff();
            };
            c_clone.send(idx);
        }
    });

    let s_clone = storage.clone();
    let consumer = thread::spawn(move || {
        for _ in 0..COUNT {
            let idx = channel.recv();
            assert!(s_clone.put(idx));
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
    assert!(storage.is_empty());
}

pub(crate) fn mpsc<S>(storage: S)
where
    S: SlotStorage + Send + 'static + Sync,
{
    let storage = Arc::new(storage);
    let channel = Arc::new(BlockingMpscChannel::new());

    let mut producers = Vec::new();
    for _ in 0..THREADS {
        let s_clone = storage.clone();
        let c_clone = channel.clone();
        producers.push(thread::spawn(move || {
            for _ in 0..COUNT {
                let idx = loop {
                    if let Some(idx) = s_clone.pull() {
                        break idx;
                    }
                    backoff();
                };
                c_clone.send(idx);
            }
        }));
    }

    let s_clone = storage.clone();
    let c_clone = channel.clone();
    let consumer = thread::spawn(move || {
        for _ in 0..(THREADS * COUNT) {
            let idx = c_clone.recv();
            assert!(s_clone.put(idx));
        }
    });

    for p in producers {
        p.join().unwrap();
    }
    consumer.join().unwrap();
    assert!(storage.is_empty());
}

pub(crate) fn mpmc<S>(storage: S)
where
    S: SlotStorage + Send + Sync + 'static,
{
    let capacity = storage.capacity();
    let storage = Arc::new(storage);

    let mut tracker = Vec::new();
    for _ in 0..capacity {
        tracker.push(Arc::new(AtomicUsize::new(0)));
    }
    let tracker = Arc::new(tracker);

    let mut workers = Vec::new();
    for thread_id in 0..THREADS {
        let s_clone = storage.clone();
        let t_clone = tracker.clone();
        workers.push(thread::spawn(move || {
            let owner_marker = thread_id + 1;
            for _ in 0..COUNT {
                let idx = loop {
                    if let Some(idx) = s_clone.pull() {
                        break idx;
                    }
                    backoff();
                };

                let old_owner = t_clone[idx].swap(owner_marker, Ordering::AcqRel);
                assert_eq!(
                    old_owner, 0,
                    "Race condition detected! Multiple threads acquired slot {}",
                    idx
                );

                backoff();

                let current_owner = t_clone[idx].load(Ordering::Acquire);
                assert_eq!(
                    current_owner, owner_marker,
                    "Slot {} was hijacked by another thread!",
                    idx
                );

                t_clone[idx].store(0, Ordering::Release);
                assert!(s_clone.put(idx));
            }
        }));
    }

    for w in workers {
        w.join().unwrap();
    }
    assert!(storage.is_empty());
}

pub(crate) fn linearizable<S>(storage: S)
where
    S: SlotStorage + Send + Sync + 'static,
{
    let storage = Arc::new(storage);

    let mut workers = Vec::new();
    for _ in 0..storage.capacity() {
        let s_clone = storage.clone();
        workers.push(thread::spawn(move || {
            for _ in 0..COUNT {
                let idx = s_clone.pull().unwrap();
                assert!(s_clone.put(idx));
            }
        }));
    }

    for w in workers {
        w.join().unwrap();
    }
}
