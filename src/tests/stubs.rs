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

pub(crate) fn batch_smoke<S>(storage: S)
where
    S: SlotPool,
{
    let cap = storage.capacity();
    assert!(storage.is_full());

    let batch = storage.pull_batch().expect("Storage should not be empty");
    let batch_count = batch.count();
    assert!(batch_count > 0 && batch_count <= 64);
    assert_eq!(storage.len(), cap - batch_count);
    assert!(!storage.is_full());

    // Put safe batch back
    assert!(storage.put_batch(batch).is_ok());
    assert_eq!(storage.len(), cap);
    assert!(storage.is_full());

    // 2. Pull raw batch
    let raw_batch = storage
        .pull_raw_batch()
        .expect("Storage should not be empty");
    assert_eq!(storage.len(), cap - raw_batch.count());

    // SAFETY:
    // we just pulled this batch from the same pool
    assert!(unsafe { storage.put_raw_batch(raw_batch) });
    assert_eq!(storage.len(), cap);

    let batch = storage.pull_batch().unwrap();

    let c = batch.count();
    let (l, r) = batch.split_at(c + 1);
    assert!(r.is_none());
    assert_eq!(l.count(), c);

    let (l, r) = l.split_at(0);
    assert_eq!(l.count(), 0);
    let r = r.unwrap();
    assert_eq!(r.count(), c);

    let (l, r) = r.split_at(c / 2);
    let r = r.unwrap();
    assert_eq!(l.count() + r.count(), c);

    assert!(storage.put_batch(r).is_ok());
    assert_eq!(storage.len(), cap - l.count());
    assert!(storage.put_batch(l).is_ok());

    assert!(storage.is_full());
}

pub(crate) fn batch_spsc<S>(storage: S)
where
    S: SlotPool + Send + Sync + 'static,
{
    use crate::Batch;

    let storage = Arc::new(storage);
    let channel = Arc::new(BlockingMpscChannel::<Batch>::new());

    let s_clone = storage.clone();
    let c_clone = channel.clone();
    let producer = thread::spawn(move || {
        let mut produced = 0;
        while produced < COUNT {
            let batch = loop {
                if let Some(b) = s_clone.pull_batch() {
                    break b;
                }
                backoff();
            };
            produced += batch.count();
            c_clone.send(batch);
        }
    });

    let s_clone = storage.clone();
    let consumer = thread::spawn(move || {
        let mut consumed = 0;
        while consumed < COUNT {
            let batch = channel.recv();
            consumed += batch.count();
            assert!(s_clone.put_batch(batch).is_ok());
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
    assert!(storage.is_full());
}

pub(crate) fn batch_mpmc<S>(storage: S)
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
            let mut processed = 0;

            while processed < COUNT {
                let batch = loop {
                    if let Some(b) = s_clone.pull_batch() {
                        break b;
                    }
                    backoff();
                };

                let batch_len = batch.count();

                // Check exclusive ownership across ALL slots in the batch
                for slot in batch.raw().into_iter() {
                    let old_owner = t_clone[slot].swap(owner_marker, Ordering::AcqRel);
                    assert_eq!(
                        old_owner, 0,
                        "Race condition! Slot {} in batch was claimed by thread {}",
                        slot, old_owner
                    );
                }

                backoff();

                // Verify no other thread hijacked any slot in the batch
                for slot in batch.raw().into_iter() {
                    let current_owner = t_clone[slot].load(Ordering::Acquire);
                    assert_eq!(
                        current_owner, owner_marker,
                        "Slot {} in batch was hijacked by thread {}!",
                        slot, current_owner
                    );
                    t_clone[slot].store(0, Ordering::Release);
                }

                assert!(s_clone.put_batch(batch).is_ok());
                processed += batch_len;
            }
        }));
    }

    for w in workers {
        w.join().unwrap();
    }
    assert!(storage.is_full());
}

/// Stress test interleaving ALL allocation paths concurrently across threads:
/// - Single pulls (`pull`)
/// - Raw single pulls (`pull_raw`)
/// - Batch pulls (`pull_batch`)
/// - Raw batch pulls (`pull_raw_batch`)
/// - Exact array pulls (`pull_exact::<2>`)
pub(crate) fn mixed_mpmc<S>(storage: S)
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

            for i in 0..COUNT {
                match i % 3 {
                    // Path 0: Single Safe Pull
                    0 => {
                        let handle = loop {
                            if let Some(h) = s_clone.pull() {
                                break h;
                            }
                            backoff();
                        };
                        let idx = handle.as_usize();

                        let old = t_clone[idx].swap(owner_marker, Ordering::AcqRel);
                        assert_eq!(old, 0, "Race condition on single pull slot {}", idx);

                        backoff();

                        assert_eq!(t_clone[idx].load(Ordering::Acquire), owner_marker);
                        t_clone[idx].store(0, Ordering::Release);
                        assert!(s_clone.put(handle).is_ok());
                    }

                    // Path 1: Safe Batch Pull
                    1 => {
                        let batch = loop {
                            if let Some(b) = s_clone.pull_batch() {
                                break b;
                            }
                            backoff();
                        };

                        for idx in batch.raw().into_iter() {
                            let old = t_clone[idx].swap(owner_marker, Ordering::AcqRel);
                            assert_eq!(old, 0, "Race condition on batch slot {}", idx);
                        }

                        backoff();

                        for idx in batch.raw().into_iter() {
                            assert_eq!(t_clone[idx].load(Ordering::Acquire), owner_marker);
                            t_clone[idx].store(0, Ordering::Release);
                        }
                        assert!(s_clone.put_batch(batch).is_ok());
                    }

                    // Path 2: Raw Batch Pull
                    _ => {
                        let raw_batch = loop {
                            if let Some(rb) = s_clone.pull_raw_batch() {
                                break rb;
                            }
                            backoff();
                        };

                        for idx in raw_batch {
                            let old = t_clone[idx].swap(owner_marker, Ordering::AcqRel);
                            assert_eq!(old, 0, "Race condition on raw batch slot {}", idx);
                        }

                        backoff();

                        for idx in raw_batch {
                            assert_eq!(t_clone[idx].load(Ordering::Acquire), owner_marker);
                            t_clone[idx].store(0, Ordering::Release);
                        }
                        // SAFETY:
                        // we pulled this batch from the same storage and free it only once, here
                        assert!(unsafe { s_clone.put_raw_batch(raw_batch) });
                    }
                }
            }
        }));
    }

    for w in workers {
        w.join().unwrap();
    }
    assert!(storage.is_full());
}
