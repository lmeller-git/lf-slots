#[cfg(feature = "alloc")]
use criterion::criterion_group;
use criterion::criterion_main;

#[cfg(feature = "alloc")]
mod a {
    use std::{sync::Arc, thread};

    use criterion::{BenchmarkId, Criterion, Throughput};
    use crossbeam_queue::ArrayQueue;
    use lf_slots::{SlotPool, Slots, cache_coherence::CoherenceProvider, core::RawSlotPool};

    const CAPACITY: usize = 4096;
    const BATCH_SIZE: usize = 32;
    const QUEUE_CAPACITY: usize = CAPACITY / BATCH_SIZE;
    // 122_880 is evenly divisible by 1, 2, 4, 8 threads AND BATCH_SIZE (32)
    const TOTAL_OPS: usize = 122_880;

    type Batch = [usize; BATCH_SIZE];

    // Trait abstraction to run identical benchmark loops over different pool types
    pub(crate) trait IndexPool: Send + Sync + 'static {
        fn pull_(&self) -> Option<usize>;
        fn put_(&self, idx: usize);
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
    }

    // =========================================================================
    // 1. Single Thread Benchmark (No Contention)
    // =========================================================================
    pub(crate) fn bench_single_thread(c: &mut Criterion) {
        let mut group = c.benchmark_group("Single Thread Throughput");
        group.throughput(Throughput::Elements(TOTAL_OPS as u64));

        group.bench_function("InlineSlots", |b| {
            b.iter(|| {
                let slots = Slots::new(CAPACITY);
                for _ in 0..TOTAL_OPS {
                    let idx = slots.pull().unwrap();
                    _ = slots.put(idx);
                }
            });
        });

        group.bench_function("ArrayQueuePool", |b| {
            b.iter(|| {
                let pool = ArrayQueuePool::new(CAPACITY);
                for _ in 0..TOTAL_OPS {
                    let idx = pool.pull_().unwrap();
                    pool.put_(idx);
                }
            });
        });

        group.finish();
    }

    // =========================================================================
    // 2. SPSC Benchmark
    // =========================================================================
    fn run_spsc<P: IndexPool>(pool: Arc<P>) {
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

    pub(crate) fn bench_spsc(c: &mut Criterion) {
        let mut group = c.benchmark_group("SPSC Throughput");
        group.throughput(Throughput::Elements(TOTAL_OPS as u64));

        group.bench_function("InlineSlots", |b| {
            b.iter(|| run_spsc(Arc::new(Slots::new(CAPACITY))));
        });

        group.bench_function("ArrayQueuePool", |b| {
            b.iter(|| run_spsc(Arc::new(ArrayQueuePool::new(CAPACITY))));
        });

        group.finish();
    }

    // =========================================================================
    // 3. MPSC Benchmark
    // =========================================================================
    fn run_mpsc<P: IndexPool>(producers: usize, pool: Arc<P>) {
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

    pub(crate) fn bench_mpsc(c: &mut Criterion) {
        let mut group = c.benchmark_group("MPSC Throughput");
        group.throughput(Throughput::Elements(TOTAL_OPS as u64));

        for num_producers in [2, 4, 8] {
            let label = format!("{} producers", num_producers);

            group.bench_with_input(
                BenchmarkId::new("InlineSlots", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| run_mpsc(producers, Arc::new(Slots::new(CAPACITY))));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| run_mpsc(producers, Arc::new(ArrayQueuePool::new(CAPACITY))));
                },
            );
        }
        group.finish();
    }

    // =========================================================================
    // 4. MPMC Benchmark
    // =========================================================================
    fn run_mpmc<P: IndexPool>(pairs: usize, pool: Arc<P>) {
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

    pub(crate) fn bench_mpmc(c: &mut Criterion) {
        let mut group = c.benchmark_group("MPMC Throughput");
        group.throughput(Throughput::Elements(TOTAL_OPS as u64));

        for thread_pairs in [1, 2, 4] {
            let total_threads = thread_pairs * 2;
            let label = format!("{} total threads", total_threads);

            group.bench_with_input(
                BenchmarkId::new("InlineSlots", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| run_mpmc(pairs, Arc::new(Slots::new(CAPACITY))));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| run_mpmc(pairs, Arc::new(ArrayQueuePool::new(CAPACITY))));
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
