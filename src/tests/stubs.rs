#![allow(dead_code)]

use alloc::{collections::VecDeque, vec::Vec};

use crate::{
    SlotPool,
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

#[cfg(loom)]
pub(crate) const COUNT: usize = 1;
#[cfg(all(any(miri, shuttle), not(loom)))]
pub(crate) const COUNT: usize = 10;
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
    S: SlotPool,
{
    let i0 = storage.pull().unwrap();
    let i1 = storage.pull().unwrap();
    assert_ne!(i0, i1);

    let i0_v = i0.as_usize();
    assert!(storage.put(i0).is_ok());
    let i2 = storage.pull().unwrap();
    assert_eq!(i0_v, i2.as_usize());
}

pub(crate) fn smoke_long<S>(storage: S)
where
    S: SlotPool,
{
    assert_eq!(storage.capacity(), 10);

    assert!(storage.is_full());
    for _ in 0..10 {
        assert!(storage.pull().is_some());
    }
    assert!(storage.is_empty());
    assert_eq!(storage.len(), 0);

    // SAFETY:
    // We use only this pool here and the pool is empty and of capacity 10
    assert!(unsafe { storage.put_raw(5) });
    assert!(!storage.is_empty());

    assert_eq!(storage.len(), 1);

    assert_eq!(storage.pull().map(|item| item.as_usize()), Some(5));

    for i in 0..10 {
        // SAFETY:
        // We use only this pool here and the pool is empty and of capacity 10
        assert!(unsafe { storage.put_raw(i) });
    }

    assert!(storage.is_full());
    assert_eq!(storage.len(), 10);

    assert_eq!(storage.pull().map(|item| item.as_usize()), Some(0));
}

pub(crate) fn len_empty_full<S>(storage: S)
where
    S: SlotPool,
{
    assert_eq!(storage.capacity(), 2);
    assert_eq!(storage.len(), 2);
    assert!(storage.is_full());
    assert!(!storage.is_empty());

    let i0 = storage.pull().unwrap();
    assert_eq!(storage.len(), 1);
    assert!(!storage.is_full());
    assert!(!storage.is_empty());

    let i1 = storage.pull().unwrap();
    assert_eq!(storage.len(), 0);
    assert!(!storage.is_full());
    assert!(storage.is_empty());

    assert!(storage.put(i0).is_ok());
    assert_eq!(storage.len(), 1);
    assert!(!storage.is_full());
    assert!(!storage.is_empty());

    assert!(storage.put(i1).is_ok());
    assert_eq!(storage.len(), 2);
    assert!(storage.is_full());
    assert!(!storage.is_empty());
}

pub(crate) struct BlockingMpscChannel<T> {
    queue: Mutex<VecDeque<T>>,
    cond: Condvar,
}

impl<T> BlockingMpscChannel<T> {
    pub(crate) fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            cond: Condvar::new(),
        }
    }

    pub(crate) fn send(&self, val: T) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(val);
        self.cond.notify_one();
    }

    pub(crate) fn recv(&self) -> T {
        let mut q = self.queue.lock().unwrap();
        while q.is_empty() {
            q = self.cond.wait(q).unwrap();
        }
        q.pop_front().unwrap()
    }
}

pub(crate) fn spsc<S>(storage: S)
where
    S: SlotPool + Send + Sync + 'static,
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
            assert!(s_clone.put(idx).is_ok());
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
    assert!(storage.is_full());
}

pub(crate) fn mpsc<S>(storage: S)
where
    S: SlotPool + Send + 'static + Sync,
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
            assert!(s_clone.put(idx).is_ok());
        }
    });

    for p in producers {
        p.join().unwrap();
    }
    consumer.join().unwrap();
    assert!(storage.is_full());
}

pub(crate) fn mpmc<S>(storage: S)
where
    S: SlotPool + Send + Sync + 'static,
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

                let old_owner = t_clone[idx.as_usize()].swap(owner_marker, Ordering::AcqRel);
                assert_eq!(
                    old_owner, 0,
                    "Race condition detected! Multiple threads acquired slot {}",
                    idx
                );

                backoff();

                let current_owner = t_clone[idx.as_usize()].load(Ordering::Acquire);
                assert_eq!(
                    current_owner, owner_marker,
                    "Slot {} was hijacked by another thread!",
                    idx
                );

                t_clone[idx.as_usize()].store(0, Ordering::Release);
                assert!(s_clone.put(idx).is_ok());
            }
        }));
    }

    for w in workers {
        w.join().unwrap();
    }
    assert!(storage.is_full());
}

pub(crate) fn linearizable<S>(storage: S)
where
    S: SlotPool + Send + Sync + 'static,
{
    let storage = Arc::new(storage);

    let mut workers = Vec::new();
    for _ in 0..storage.capacity() {
        let s_clone = storage.clone();
        workers.push(thread::spawn(move || {
            for _ in 0..COUNT {
                let idx = s_clone.pull().unwrap();
                assert!(s_clone.put(idx).is_ok());
            }
        }));
    }

    for w in workers {
        w.join().unwrap();
    }
}
