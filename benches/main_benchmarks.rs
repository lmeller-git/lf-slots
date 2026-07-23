#[cfg(feature = "alloc")]
use criterion::criterion_group;
use criterion::criterion_main;

#[cfg(feature = "alloc")]
mod a {
    use std::{sync::Arc, thread};

    use criterion::{BenchmarkId, Criterion, Throughput};
    use crossbeam_queue::ArrayQueue;
    use crossbeam_utils::CachePadded;
    use lf_slots::{
        SlotPool,
        Slots,
        cache_coherence::CoherenceProvider,
        core::{RawSlotPool, Word},
    };

    const CAPACITY: usize = 4096;
    const BATCH_SIZE: usize =
        align_of::<CachePadded<()>>() / size_of::<Word>() * Word::BITS as usize / 2;
    const QUEUE_CAPACITY: usize = CAPACITY / BATCH_SIZE;
    // 122_880 is evenly divisible by 1, 2, 4, 8 threads AND BATCH_SIZE (32)
    const TOTAL_OPS: usize = 122_880;

    type Batch = [usize; BATCH_SIZE];

    // Trait abstraction to run identical benchmark loops over different pool types
    pub(crate) trait IndexPool: Send + Sync + 'static {
        fn pull_(&self) -> Option<usize>;
        fn put_(&self, idx: usize);

        fn pull_exact_(&self) -> Option<Batch>;
        fn put_batch_(&self, batch: Batch);
    }

    impl<C: CoherenceProvider + Send + Sync + 'static> IndexPool for Slots<C> {
        #[inline]
        fn pull_(&self) -> Option<usize> {
            SlotPool::pull(self).map(|i| i.as_usize())
        }

        #[inline]
        fn put_(&self, idx: usize) {
            unsafe { self.put_raw(idx) };
        }

        #[inline]
        fn pull_exact_(&self) -> Option<Batch> {
            SlotPool::pull_exact::<BATCH_SIZE>(self)
                .map(|handles| handles.map(|handle| handle.as_usize()))
        }

        #[inline]
        fn put_batch_(&self, batch: Batch) {
            for idx in batch {
                unsafe { self.put_raw(idx) };
            }
        }
    }

    /// Comparison pool: Pre-populates ArrayQueue with 0..CAPACITY index tokens
    pub(crate) struct ArrayQueuePool {
        queue: ArrayQueue<usize>,
    }

    impl ArrayQueuePool {
        pub(crate) fn new(capacity: usize) -> Self {
            let queue = ArrayQueue::new(capacity);
            for i in 0..capacity {
                let _ = queue.push(i);
            }
            Self { queue }
        }
    }

    impl IndexPool for ArrayQueuePool {
        #[inline]
        fn pull_(&self) -> Option<usize> {
            self.queue.pop()
        }

        #[inline]
        fn put_(&self, idx: usize) {
            let _ = self.queue.push(idx);
        }

        #[inline]
        fn pull_exact_(&self) -> Option<Batch> {
            let mut batch = [0usize; BATCH_SIZE];
            for i in 0..BATCH_SIZE {
                if let Some(idx) = self.queue.pop() {
                    batch[i] = idx;
                } else {
                    // Rollback acquired items on partial pool depletion
                    for j in 0..i {
                        let _ = self.queue.push(batch[j]);
                    }
                    return None;
                }
            }
            Some(batch)
        }

        #[inline]
        fn put_batch_(&self, batch: Batch) {
            for idx in batch {
                let _ = self.queue.push(idx);
            }
        }
    }

    // =========================================================================
    // 1. Single Thread Benchmark (No Contention)
    // =========================================================================
    pub(crate) fn bench_single_thread(c: &mut Criterion) {
        let mut group = c.benchmark_group("Single Thread Throughput");
        group.throughput(Throughput::Elements(TOTAL_OPS as u64));

        group.bench_function("InlineSlots (Scalar)", |b| {
            b.iter(|| {
                let slots = Slots::new(CAPACITY);
                for _ in 0..TOTAL_OPS {
                    let idx = slots.pull().unwrap();
                    _ = slots.put(idx);
                }
            });
        });

        group.bench_function("InlineSlots (pull_exact)", |b| {
            b.iter(|| {
                let slots = Slots::new(CAPACITY);
                let total_batches = TOTAL_OPS / BATCH_SIZE;
                for _ in 0..total_batches {
                    let batch = slots.pull_exact_().unwrap();
                    slots.put_batch_(batch);
                }
            });
        });

        group.bench_function("ArrayQueuePool (Scalar)", |b| {
            b.iter(|| {
                let pool = ArrayQueuePool::new(CAPACITY);
                for _ in 0..TOTAL_OPS {
                    let idx = pool.pull_().unwrap();
                    pool.put_(idx);
                }
            });
        });

        group.bench_function("ArrayQueuePool (Batch)", |b| {
            b.iter(|| {
                let pool = ArrayQueuePool::new(CAPACITY);
                let total_batches = TOTAL_OPS / BATCH_SIZE;
                for _ in 0..total_batches {
                    let batch = pool.pull_exact_().unwrap();
                    pool.put_batch_(batch);
                }
            });
        });

        group.finish();
    }

    // =========================================================================
    // 2. SPSC Benchmark
    // =========================================================================
    fn run_spsc_scalar<P: IndexPool>(pool: Arc<P>) {
        let queue = Arc::new(ArrayQueue::<Batch>::new(QUEUE_CAPACITY));

        let p_clone = pool.clone();
        let q_clone = queue.clone();
        let producer = thread::spawn(move || {
            let total_batches = TOTAL_OPS / BATCH_SIZE;
            for _ in 0..total_batches {
                let mut chunk = [0usize; BATCH_SIZE];
                for slot in chunk.iter_mut() {
                    *slot = loop {
                        if let Some(idx) = p_clone.pull_() {
                            break idx;
                        }
                        std::hint::spin_loop();
                    };
                }
                while let Err(c) = q_clone.push(chunk) {
                    chunk = c;
                    std::hint::spin_loop();
                }
            }
        });

        let consumer = thread::spawn(move || {
            let total_batches = TOTAL_OPS / BATCH_SIZE;
            for _ in 0..total_batches {
                let chunk = loop {
                    if let Some(c) = queue.pop() {
                        break c;
                    }
                    std::hint::spin_loop();
                };
                for idx in chunk {
                    pool.put_(idx);
                }
            }
        });

        producer.join().unwrap();
        consumer.join().unwrap();
    }

    fn run_spsc_exact<P: IndexPool>(pool: Arc<P>) {
        let queue = Arc::new(ArrayQueue::<Batch>::new(QUEUE_CAPACITY));

        let p_clone = pool.clone();
        let q_clone = queue.clone();
        let producer = thread::spawn(move || {
            let total_batches = TOTAL_OPS / BATCH_SIZE;
            for _ in 0..total_batches {
                let chunk = loop {
                    if let Some(b) = p_clone.pull_exact_() {
                        break b;
                    }
                    std::hint::spin_loop();
                };
                let mut push_chunk = chunk;
                while let Err(c) = q_clone.push(push_chunk) {
                    push_chunk = c;
                    std::hint::spin_loop();
                }
            }
        });

        let consumer = thread::spawn(move || {
            let total_batches = TOTAL_OPS / BATCH_SIZE;
            for _ in 0..total_batches {
                let chunk = loop {
                    if let Some(c) = queue.pop() {
                        break c;
                    }
                    std::hint::spin_loop();
                };
                pool.put_batch_(chunk);
            }
        });

        producer.join().unwrap();
        consumer.join().unwrap();
    }

    pub(crate) fn bench_spsc(c: &mut Criterion) {
        let mut group = c.benchmark_group("SPSC Throughput");
        group.throughput(Throughput::Elements(TOTAL_OPS as u64));

        group.bench_function("InlineSlots (Scalar)", |b| {
            b.iter(|| run_spsc_scalar(Arc::new(Slots::new(CAPACITY))));
        });

        group.bench_function("InlineSlots (pull_exact)", |b| {
            b.iter(|| run_spsc_exact(Arc::new(Slots::new(CAPACITY))));
        });

        group.bench_function("ArrayQueuePool (Scalar)", |b| {
            b.iter(|| run_spsc_scalar(Arc::new(ArrayQueuePool::new(CAPACITY))));
        });

        group.bench_function("ArrayQueuePool (Batch)", |b| {
            b.iter(|| run_spsc_exact(Arc::new(ArrayQueuePool::new(CAPACITY))));
        });

        group.finish();
    }

    // =========================================================================
    // 3. MPSC Benchmark
    // =========================================================================
    fn run_mpsc_scalar<P: IndexPool>(producers: usize, pool: Arc<P>) {
        let queue = Arc::new(ArrayQueue::<Batch>::new(QUEUE_CAPACITY));
        let batches_per_producer = (TOTAL_OPS / producers) / BATCH_SIZE;

        let mut producer_handles = Vec::new();
        for _ in 0..producers {
            let p_clone = pool.clone();
            let q_clone = queue.clone();
            producer_handles.push(thread::spawn(move || {
                for _ in 0..batches_per_producer {
                    let mut chunk = [0usize; BATCH_SIZE];
                    for slot in chunk.iter_mut() {
                        *slot = loop {
                            if let Some(idx) = p_clone.pull_() {
                                break idx;
                            }
                            std::hint::spin_loop();
                        };
                    }
                    while let Err(c) = q_clone.push(chunk) {
                        chunk = c;
                        std::hint::spin_loop();
                    }
                }
            }));
        }

        let p_clone = pool.clone();
        let consumer = thread::spawn(move || {
            let total_batches = TOTAL_OPS / BATCH_SIZE;
            for _ in 0..total_batches {
                let chunk = loop {
                    if let Some(c) = queue.pop() {
                        break c;
                    }
                    std::hint::spin_loop();
                };
                for idx in chunk {
                    p_clone.put_(idx);
                }
            }
        });

        for p in producer_handles {
            p.join().unwrap();
        }
        consumer.join().unwrap();
    }

    fn run_mpsc_exact<P: IndexPool>(producers: usize, pool: Arc<P>) {
        let queue = Arc::new(ArrayQueue::<Batch>::new(QUEUE_CAPACITY));
        let batches_per_producer = (TOTAL_OPS / producers) / BATCH_SIZE;

        let mut producer_handles = Vec::new();
        for _ in 0..producers {
            let p_clone = pool.clone();
            let q_clone = queue.clone();
            producer_handles.push(thread::spawn(move || {
                for _ in 0..batches_per_producer {
                    let chunk = loop {
                        if let Some(b) = p_clone.pull_exact_() {
                            break b;
                        }
                        std::hint::spin_loop();
                    };
                    let mut push_chunk = chunk;
                    while let Err(c) = q_clone.push(push_chunk) {
                        push_chunk = c;
                        std::hint::spin_loop();
                    }
                }
            }));
        }

        let p_clone = pool.clone();
        let consumer = thread::spawn(move || {
            let total_batches = TOTAL_OPS / BATCH_SIZE;
            for _ in 0..total_batches {
                let chunk = loop {
                    if let Some(c) = queue.pop() {
                        break c;
                    }
                    std::hint::spin_loop();
                };
                p_clone.put_batch_(chunk);
            }
        });

        for p in producer_handles {
            p.join().unwrap();
        }
        consumer.join().unwrap();
    }

    pub(crate) fn bench_mpsc(c: &mut Criterion) {
        let mut group = c.benchmark_group("MPSC Throughput");
        group.throughput(Throughput::Elements(TOTAL_OPS as u64));

        for num_producers in [2, 4, 8] {
            let label = format!("{} producers", num_producers);

            group.bench_with_input(
                BenchmarkId::new("InlineSlots (Scalar)", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| run_mpsc_scalar(producers, Arc::new(Slots::new(CAPACITY))));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("InlineSlots (pull_exact)", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| run_mpsc_exact(producers, Arc::new(Slots::new(CAPACITY))));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool (Scalar)", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| run_mpsc_scalar(producers, Arc::new(ArrayQueuePool::new(CAPACITY))));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool (Batch)", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| run_mpsc_exact(producers, Arc::new(ArrayQueuePool::new(CAPACITY))));
                },
            );
        }
        group.finish();
    }

    // =========================================================================
    // 4. MPMC Benchmark
    // =========================================================================
    fn run_mpmc_scalar<P: IndexPool>(pairs: usize, pool: Arc<P>) {
        let queue = Arc::new(ArrayQueue::<Batch>::new(QUEUE_CAPACITY));
        let batches_per_thread = (TOTAL_OPS / pairs) / BATCH_SIZE;

        let mut handles = Vec::new();

        // Producers
        for _ in 0..pairs {
            let p_clone = pool.clone();
            let q_clone = queue.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..batches_per_thread {
                    let mut chunk = [0usize; BATCH_SIZE];
                    for slot in chunk.iter_mut() {
                        *slot = loop {
                            if let Some(idx) = p_clone.pull_() {
                                break idx;
                            }
                            std::hint::spin_loop();
                        };
                    }
                    while let Err(c) = q_clone.push(chunk) {
                        chunk = c;
                        std::hint::spin_loop();
                    }
                }
            }));
        }

        // Consumers
        for _ in 0..pairs {
            let p_clone = pool.clone();
            let q_clone = queue.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..batches_per_thread {
                    let chunk = loop {
                        if let Some(c) = q_clone.pop() {
                            break c;
                        }
                        std::hint::spin_loop();
                    };
                    for idx in chunk {
                        p_clone.put_(idx);
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }

    fn run_mpmc_exact<P: IndexPool>(pairs: usize, pool: Arc<P>) {
        let queue = Arc::new(ArrayQueue::<Batch>::new(QUEUE_CAPACITY));
        let batches_per_thread = (TOTAL_OPS / pairs) / BATCH_SIZE;

        let mut handles = Vec::new();

        // Producers
        for _ in 0..pairs {
            let p_clone = pool.clone();
            let q_clone = queue.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..batches_per_thread {
                    let chunk = loop {
                        if let Some(b) = p_clone.pull_exact_() {
                            break b;
                        }
                        std::hint::spin_loop();
                    };
                    let mut push_chunk = chunk;
                    while let Err(c) = q_clone.push(push_chunk) {
                        push_chunk = c;
                        std::hint::spin_loop();
                    }
                }
            }));
        }

        // Consumers
        for _ in 0..pairs {
            let p_clone = pool.clone();
            let q_clone = queue.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..batches_per_thread {
                    let chunk = loop {
                        if let Some(c) = q_clone.pop() {
                            break c;
                        }
                        std::hint::spin_loop();
                    };
                    p_clone.put_batch_(chunk);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }

    pub(crate) fn bench_mpmc(c: &mut Criterion) {
        let mut group = c.benchmark_group("MPMC Throughput");
        group.throughput(Throughput::Elements(TOTAL_OPS as u64));

        for thread_pairs in [1, 2, 4] {
            let total_threads = thread_pairs * 2;
            let label = format!("{} total threads", total_threads);

            group.bench_with_input(
                BenchmarkId::new("InlineSlots (Scalar)", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| run_mpmc_scalar(pairs, Arc::new(Slots::new(CAPACITY))));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("InlineSlots (pull_exact)", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| run_mpmc_exact(pairs, Arc::new(Slots::new(CAPACITY))));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool (Scalar)", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| run_mpmc_scalar(pairs, Arc::new(ArrayQueuePool::new(CAPACITY))));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool (Batch)", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| run_mpmc_exact(pairs, Arc::new(ArrayQueuePool::new(CAPACITY))));
                },
            );
        }
        group.finish();
    }
}

#[cfg(feature = "alloc")]
use a::{bench_mpmc, bench_mpsc, bench_single_thread, bench_spsc};

#[cfg(feature = "alloc")]
criterion_group!(
    benches,
    bench_single_thread,
    bench_spsc,
    bench_mpsc,
    bench_mpmc
);

#[cfg(feature = "alloc")]
criterion_main!(benches);

#[cfg(not(feature = "alloc"))]
fn foo() {}

#[cfg(not(feature = "alloc"))]
criterion_main!(foo);
