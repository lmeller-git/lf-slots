//! Benchmark suite for `lf_slots`, organized into four independently
//! filterable groups so you don't have to run everything on every change:
//!
//!   1. `single_thread_benches` -- uncontended, straight-line throughput
//!   2. `concurrent_benches`    -- SPSC / MPSC / MPMC sweep
//!   3. `small_capacity_benches`-- tiny-capacity contention strategies
//!   4. `api_tier_benches`     -- pure single-op instruction-cost comparison

#[cfg(feature = "alloc")]
use criterion::criterion_group;
use criterion::criterion_main;

// =========================================================================
// Shared harness: IndexPool abstraction + generic concurrent runner.
// Powers groups 1 and 2. Groups 3 and 4 are narrower, more hardware-level
// comparisons and stay hand-rolled against concrete types (see their
// module docs) -- forcing them through the same generic machinery would
// obscure what they're isolating, not clarify it.
// =========================================================================
#[cfg(feature = "alloc")]
mod common {
    use std::{sync::Arc, thread};

    use crossbeam_queue::ArrayQueue;
    use crossbeam_utils::CachePadded;
    use lf_slots::{
        BatchedSlotPool,
        SlotPool,
        Slots,
        cache_coherence::CoherenceProvider,
        core::{BatchedRawSlotPool, RawBatch, RawSlotPool, Word},
    };

    pub(crate) const CAPACITY: usize = 4096;
    pub(crate) const BATCH_SIZE: usize =
        align_of::<CachePadded<()>>() / size_of::<Word>() * Word::BITS as usize / 2;
    pub(crate) const QUEUE_CAPACITY: usize = CAPACITY / BATCH_SIZE;
    // 122_880 divides evenly by 1/2/4/8 threads, by BATCH_SIZE, and by
    // Word::BITS
    pub(crate) const TOTAL_OPS: usize = 122_880;

    /// A fixed-size array of individually-decomposed slot indices -- what
    /// `pull_exact` returns (as usize) and what looped `put_raw` expects.
    /// Named `ExactBatch` rather than plain `Batch` to avoid colliding
    /// with the crate's own `lf_slots::core::Batch` (the safe wrapper
    /// around `RawBatch`), which isn't used directly in this file but is
    /// visible in your pasted API surface.
    pub(crate) type ExactBatch = [usize; BATCH_SIZE];

    // ---- Sizing for the raw word-batch tier --------------------------------
    pub(crate) const WORD_BITS: usize = Word::BITS as usize;
    pub(crate) const WORD_QUEUE_CAPACITY: usize = CAPACITY / WORD_BITS;
    pub(crate) const TOTAL_RAW_BATCHES: usize = TOTAL_OPS / WORD_BITS;

    /// Trait abstraction to run identical benchmark loops over different
    /// pool types. Every method is suffixed with `_` to avoid name
    /// collisions with the inherent `SlotPool`/`RawSlotPool` trait
    /// methods these wrap (see e.g. `pull_` vs. `SlotPool::pull`).
    pub(crate) trait IndexPool: Send + Sync + 'static {
        fn pull_(&self) -> Option<usize>;
        fn put_(&self, idx: usize);

        fn pull_exact_(&self) -> Option<ExactBatch>;
        fn put_exact_(&self, batch: ExactBatch);
    }

    /// Extension for pool types that expose the raw word-batch primitive
    /// (`pull_raw_batch`/`put_raw_batch`, a `RawSlotPool` method): one
    /// atomic op claims/releases up to `Word::BITS` slots at once.
    /// `ArrayQueuePool` has no equivalent concept, so it only implements
    /// plain `IndexPool`.
    pub(crate) trait RawBatchPool_: IndexPool {
        fn pull_raw_batch_(&self) -> Option<RawBatch>;
        fn put_raw_batch_(&self, batch: RawBatch);
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
        fn pull_exact_(&self) -> Option<ExactBatch> {
            BatchedSlotPool::pull_exact::<BATCH_SIZE>(self)
                .map(|handles| handles.map(|handle| handle.as_usize()))
        }

        #[inline]
        fn put_exact_(&self, batch: ExactBatch) {
            for idx in batch {
                unsafe { self.put_raw(idx) };
            }
        }
    }

    impl<C: CoherenceProvider + Send + Sync + 'static> RawBatchPool_ for Slots<C> {
        #[inline]
        fn pull_raw_batch_(&self) -> Option<RawBatch> {
            self.pull_raw_batch()
        }

        #[inline]
        fn put_raw_batch_(&self, batch: RawBatch) {
            unsafe { self.put_raw_batch(batch) };
        }
    }

    /// Comparison pool: pre-populates ArrayQueue with 0..CAPACITY index tokens.
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
        fn pull_exact_(&self) -> Option<ExactBatch> {
            let mut batch = [0usize; BATCH_SIZE];
            for i in 0..BATCH_SIZE {
                if let Some(idx) = self.queue.pop() {
                    batch[i] = idx;
                } else {
                    // Rollback acquired items on partial pool depletion.
                    for j in 0..i {
                        let _ = self.queue.push(batch[j]);
                    }
                    return None;
                }
            }
            Some(batch)
        }

        #[inline]
        fn put_exact_(&self, batch: ExactBatch) {
            for idx in batch {
                let _ = self.queue.push(idx);
            }
        }
    }

    // ---- Acquire/release helpers, one pair per API tier --------------------
    // These are what actually get timed inside `run_concurrent`'s
    // producer/consumer loops; each pairs with the `Item` type moved over
    // the handoff queue (a fixed-size `ExactBatch` array for Scalar/Exact,
    // a single opaque `RawBatch` for the raw tier).

    pub(crate) fn scalar_acquire<P: IndexPool>(pool: &P) -> ExactBatch {
        let mut batch = [0usize; BATCH_SIZE];
        for slot in batch.iter_mut() {
            *slot = loop {
                if let Some(idx) = pool.pull_() {
                    break idx;
                }
                std::hint::spin_loop();
            };
        }
        batch
    }

    pub(crate) fn scalar_release<P: IndexPool>(pool: &P, batch: ExactBatch) {
        for idx in batch {
            pool.put_(idx);
        }
    }

    pub(crate) fn exact_acquire<P: IndexPool>(pool: &P) -> ExactBatch {
        loop {
            if let Some(batch) = pool.pull_exact_() {
                return batch;
            }
            std::hint::spin_loop();
        }
    }

    pub(crate) fn exact_release<P: IndexPool>(pool: &P, batch: ExactBatch) {
        pool.put_exact_(batch);
    }

    pub(crate) fn raw_acquire<P: RawBatchPool_>(pool: &P) -> RawBatch {
        loop {
            if let Some(batch) = pool.pull_raw_batch_() {
                return batch;
            }
            std::hint::spin_loop();
        }
    }

    pub(crate) fn raw_release<P: RawBatchPool_>(pool: &P, batch: RawBatch) {
        pool.put_raw_batch_(batch);
    }

    /// Generic N-producer / M-consumer throughput runner shared by SPSC,
    /// MPSC and MPMC (SPSC is `producers = consumers = 1`; MPSC is
    /// `consumers = 1`) across all three API tiers and both pool types --
    /// only `acquire`/`release` and the `Item` transported over the
    /// handoff queue change between call sites.
    pub(crate) fn run_concurrent<P, Item, Acq, Rel>(
        producers: usize,
        consumers: usize,
        total_items: usize,
        queue_capacity: usize,
        pool: Arc<P>,
        acquire: Acq,
        release: Rel,
    ) where
        P: Send + Sync + 'static,
        Item: Send + 'static,
        Acq: Fn(&P) -> Item + Copy + Send + 'static,
        Rel: Fn(&P, Item) + Copy + Send + 'static,
    {
        let queue = Arc::new(ArrayQueue::<Item>::new(queue_capacity));
        let items_per_producer = total_items / producers;
        let items_per_consumer = total_items / consumers;

        let mut handles = Vec::with_capacity(producers + consumers);

        for _ in 0..producers {
            let pool = pool.clone();
            let queue = queue.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..items_per_producer {
                    let mut item = acquire(&pool);
                    while let Err(returned) = queue.push(item) {
                        item = returned;
                        std::hint::spin_loop();
                    }
                }
            }));
        }

        for _ in 0..consumers {
            let pool = pool.clone();
            let queue = queue.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..items_per_consumer {
                    let item = loop {
                        if let Some(i) = queue.pop() {
                            break i;
                        }
                        std::hint::spin_loop();
                    };
                    release(&pool, item);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}

// =========================================================================
// Group 1: single-threaded, zero-contention throughput. Establishes the
// per-tier/per-implementation baseline that groups 2/3 build contention
// on top of.
// =========================================================================
#[cfg(feature = "alloc")]
mod single_thread {
    use criterion::{Criterion, Throughput};
    use lf_slots::{SlotPool, Slots, core::BatchedRawSlotPool};

    use crate::common::{
        ArrayQueuePool,
        BATCH_SIZE,
        CAPACITY,
        IndexPool,
        TOTAL_OPS,
        TOTAL_RAW_BATCHES,
    };

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
                    slots.put_exact_(batch);
                }
            });
        });

        group.bench_function("InlineSlots (pull_raw_batch)", |b| {
            b.iter(|| {
                let slots = Slots::new(CAPACITY);
                for _ in 0..TOTAL_RAW_BATCHES {
                    let batch = slots.pull_raw_batch().unwrap();
                    unsafe {
                        slots.put_raw_batch(batch);
                    }
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
                    pool.put_exact_(batch);
                }
            });
        });

        group.finish();
    }
}

// =========================================================================
// Group 2: SPSC / MPSC / MPMC throughput sweep. Same three group names as
// your original file (so Criterion's saved history for the existing
// Scalar/Exact benches keeps comparing against itself), now powered by
// `common::run_concurrent` and extended with the raw-batch tier.
// =========================================================================
#[cfg(feature = "alloc")]
mod concurrent {
    use std::sync::Arc;

    use criterion::{BenchmarkId, Criterion, Throughput};
    use lf_slots::Slots;

    use crate::common::{
        ArrayQueuePool,
        BATCH_SIZE,
        CAPACITY,
        QUEUE_CAPACITY,
        TOTAL_OPS,
        TOTAL_RAW_BATCHES,
        WORD_QUEUE_CAPACITY,
        exact_acquire,
        exact_release,
        raw_acquire,
        raw_release,
        run_concurrent,
        scalar_acquire,
        scalar_release,
    };

    pub(crate) fn bench_spsc(c: &mut Criterion) {
        let mut group = c.benchmark_group("SPSC Throughput");
        group.throughput(Throughput::Elements(TOTAL_OPS as u64));

        group.bench_function("InlineSlots (Scalar)", |b| {
            b.iter(|| {
                run_concurrent(
                    1,
                    1,
                    TOTAL_OPS / BATCH_SIZE,
                    QUEUE_CAPACITY,
                    Arc::new(Slots::new(CAPACITY)),
                    scalar_acquire,
                    scalar_release,
                );
            });
        });

        group.bench_function("InlineSlots (pull_exact)", |b| {
            b.iter(|| {
                run_concurrent(
                    1,
                    1,
                    TOTAL_OPS / BATCH_SIZE,
                    QUEUE_CAPACITY,
                    Arc::new(Slots::new(CAPACITY)),
                    exact_acquire,
                    exact_release,
                );
            });
        });

        group.bench_function("InlineSlots (pull_raw_batch)", |b| {
            b.iter(|| {
                run_concurrent(
                    1,
                    1,
                    TOTAL_RAW_BATCHES,
                    WORD_QUEUE_CAPACITY,
                    Arc::new(Slots::new(CAPACITY)),
                    raw_acquire,
                    raw_release,
                );
            });
        });

        group.bench_function("ArrayQueuePool (Scalar)", |b| {
            b.iter(|| {
                run_concurrent(
                    1,
                    1,
                    TOTAL_OPS / BATCH_SIZE,
                    QUEUE_CAPACITY,
                    Arc::new(ArrayQueuePool::new(CAPACITY)),
                    scalar_acquire,
                    scalar_release,
                );
            });
        });

        group.bench_function("ArrayQueuePool (Batch)", |b| {
            b.iter(|| {
                run_concurrent(
                    1,
                    1,
                    TOTAL_OPS / BATCH_SIZE,
                    QUEUE_CAPACITY,
                    Arc::new(ArrayQueuePool::new(CAPACITY)),
                    exact_acquire,
                    exact_release,
                );
            });
        });

        group.finish();
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
                    b.iter(|| {
                        run_concurrent(
                            producers,
                            1,
                            TOTAL_OPS / BATCH_SIZE,
                            QUEUE_CAPACITY,
                            Arc::new(Slots::new(CAPACITY)),
                            scalar_acquire,
                            scalar_release,
                        );
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("InlineSlots (pull_exact)", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| {
                        run_concurrent(
                            producers,
                            1,
                            TOTAL_OPS / BATCH_SIZE,
                            QUEUE_CAPACITY,
                            Arc::new(Slots::new(CAPACITY)),
                            exact_acquire,
                            exact_release,
                        );
                    });
                },
            );

            // NEW
            group.bench_with_input(
                BenchmarkId::new("InlineSlots (pull_raw_batch)", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| {
                        run_concurrent(
                            producers,
                            1,
                            TOTAL_RAW_BATCHES,
                            WORD_QUEUE_CAPACITY,
                            Arc::new(Slots::new(CAPACITY)),
                            raw_acquire,
                            raw_release,
                        );
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool (Scalar)", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| {
                        run_concurrent(
                            producers,
                            1,
                            TOTAL_OPS / BATCH_SIZE,
                            QUEUE_CAPACITY,
                            Arc::new(ArrayQueuePool::new(CAPACITY)),
                            scalar_acquire,
                            scalar_release,
                        );
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool (Batch)", &label),
                &num_producers,
                |b, &producers| {
                    b.iter(|| {
                        run_concurrent(
                            producers,
                            1,
                            TOTAL_OPS / BATCH_SIZE,
                            QUEUE_CAPACITY,
                            Arc::new(ArrayQueuePool::new(CAPACITY)),
                            exact_acquire,
                            exact_release,
                        );
                    });
                },
            );
        }
        group.finish();
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
                    b.iter(|| {
                        run_concurrent(
                            pairs,
                            pairs,
                            TOTAL_OPS / BATCH_SIZE,
                            QUEUE_CAPACITY,
                            Arc::new(Slots::new(CAPACITY)),
                            scalar_acquire,
                            scalar_release,
                        );
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("InlineSlots (pull_exact)", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| {
                        run_concurrent(
                            pairs,
                            pairs,
                            TOTAL_OPS / BATCH_SIZE,
                            QUEUE_CAPACITY,
                            Arc::new(Slots::new(CAPACITY)),
                            exact_acquire,
                            exact_release,
                        );
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("InlineSlots (pull_raw_batch)", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| {
                        run_concurrent(
                            pairs,
                            pairs,
                            TOTAL_RAW_BATCHES,
                            WORD_QUEUE_CAPACITY,
                            Arc::new(Slots::new(CAPACITY)),
                            raw_acquire,
                            raw_release,
                        );
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool (Scalar)", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| {
                        run_concurrent(
                            pairs,
                            pairs,
                            TOTAL_OPS / BATCH_SIZE,
                            QUEUE_CAPACITY,
                            Arc::new(ArrayQueuePool::new(CAPACITY)),
                            scalar_acquire,
                            scalar_release,
                        );
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ArrayQueuePool (Batch)", &label),
                &thread_pairs,
                |b, &pairs| {
                    b.iter(|| {
                        run_concurrent(
                            pairs,
                            pairs,
                            TOTAL_OPS / BATCH_SIZE,
                            QUEUE_CAPACITY,
                            Arc::new(ArrayQueuePool::new(CAPACITY)),
                            exact_acquire,
                            exact_release,
                        );
                    });
                },
            );
        }
        group.finish();
    }
}

// =========================================================================
// Group 3: tiny-capacity, high-contention strategies (rewritten from the
// original `tiny_bench::bench_tiny_cap_sharding`; kept the same N-thread
// symmetric-churn shape since that's the right shape for this question --
// every thread both allocates and frees against the *same* pool, so
// cross-core cacheline contention is what's on trial, not a producer/
// consumer split). Extended with a third arm testing spec item (c):
// WordSlots at matched logical capacity.
// =========================================================================
#[cfg(all(feature = "alloc", feature = "word-slots"))]
mod small_capacity {
    use std::{sync::Arc, thread};

    use criterion::{BenchmarkId, Criterion, Throughput};
    use lf_slots::{
        SlotPool,
        core::{RawSlotPool, WORDS_PER_CACHE_LINE},
        define_inline_slots,
        define_inline_wordslots,
    };

    // 128 allows 2 shards for the adaptive slots
    const TINY_CAP: usize = 128;

    // Adaptive macro: automatically scales down shard size under tiny
    // capacity -- kept from the original suite.
    define_inline_slots!(AdaptiveSlots32, TINY_CAP);

    // Fixed single shard: forces all 32 slots into a single atomic CAS
    // target -- the worst-case packing.
    define_inline_slots!(SingleShardSlots32, TINY_CAP, WORDS_PER_CACHE_LINE);

    // WordSlots-backed: TINY_CAP independently-owned words, i.e. TINY_CAP
    // claimable units each on its own cacheline -- the strategy from spec
    // item (c). Comparable to the two bitset strategies above 1-for-1
    // since all three offer exactly TINY_CAP claimable units
    define_inline_wordslots!(TinyWordSlots32, TINY_CAP);

    pub(crate) fn bench_small_capacity_strategies(c: &mut Criterion) {
        let mut group = c.benchmark_group("Small Capacity Strategies (N = 128)");
        let total_ops = 64_000;
        group.throughput(Throughput::Elements(total_ops as u64));

        for num_threads in [4, 8, 16] {
            let ops_per_thread = total_ops / num_threads;
            let label = format!("{} threads", num_threads);

            group.bench_with_input(
                BenchmarkId::new("Adaptive (Multi-Shard)", &label),
                &num_threads,
                |b, &threads| {
                    b.iter(|| {
                        let pool = Arc::new(AdaptiveSlots32::new());
                        let handles: Vec<_> = (0..threads)
                            .map(|_| {
                                let p = pool.clone();
                                thread::spawn(move || {
                                    for _ in 0..ops_per_thread {
                                        let idx = loop {
                                            if let Some(i) = p.pull() {
                                                break i;
                                            }
                                            std::hint::spin_loop();
                                        };
                                        unsafe {
                                            p.put_raw(idx.as_usize());
                                        }
                                    }
                                })
                            })
                            .collect();
                        for h in handles {
                            h.join().unwrap();
                        }
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("Forced Single-Shard (1 CAS Target)", &label),
                &num_threads,
                |b, &threads| {
                    b.iter(|| {
                        let pool = Arc::new(SingleShardSlots32::new());
                        let handles: Vec<_> = (0..threads)
                            .map(|_| {
                                let p = pool.clone();
                                thread::spawn(move || {
                                    for _ in 0..ops_per_thread {
                                        let idx = loop {
                                            if let Some(i) = p.pull() {
                                                break i;
                                            }
                                            std::hint::spin_loop();
                                        };
                                        unsafe {
                                            p.put_raw(idx.as_usize());
                                        }
                                    }
                                })
                            })
                            .collect();
                        for h in handles {
                            h.join().unwrap();
                        }
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("WordSlots (Distributed Cachelines)", &label),
                &num_threads,
                |b, &threads| {
                    b.iter(|| {
                        let pool = Arc::new(TinyWordSlots32::new());
                        let handles: Vec<_> = (0..threads)
                            .map(|_| {
                                let p = pool.clone();
                                thread::spawn(move || {
                                    for _ in 0..ops_per_thread {
                                        let idx = loop {
                                            if let Some(i) = p.pull() {
                                                break i;
                                            }
                                            std::hint::spin_loop();
                                        };
                                        unsafe {
                                            p.put_raw(idx.as_usize());
                                        }
                                    }
                                })
                            })
                            .collect();
                        for h in handles {
                            h.join().unwrap();
                        }
                    });
                },
            );
        }

        group.finish();
    }
}

// =========================================================================
// Group 4: pure single-threaded, single-op instruction/CAS-count cost
// comparison. Isolates API-tier cost delta (scalar vs. raw-batch on identical
// storage) from storage-implementation delta (generic bitset vs. purpose-built
// WordSlots).
// =========================================================================
#[cfg(all(feature = "alloc", feature = "word-slots"))]
mod api_tier_cost {
    use criterion::{Criterion, Throughput};
    use lf_slots::{BatchedSlotPool, SlotPool, define_inline_slots, define_inline_wordslots};

    use crate::common::{ArrayQueuePool, IndexPool};

    const CAP: usize = 32;
    const ITERS: usize = 400;

    define_inline_slots!(InlineSlots4, CAP);
    define_inline_wordslots!(InlineWordSlots4, CAP);

    pub(crate) fn bench_word_granularity(c: &mut Criterion) {
        let mut group = c.benchmark_group("API Tier Instruction Cost (Raw vs Scalar)");
        let total_slots = (CAP * ITERS) as u64;
        group.throughput(Throughput::Elements(total_slots));

        group.bench_function("InlineSlots (Scalar Pulls)", |b| {
            b.iter(|| {
                let pool = InlineSlots4::new();
                let mut handles = Vec::with_capacity(CAP);
                for _ in 0..ITERS {
                    for _ in 0..CAP {
                        if let Some(h) = pool.pull() {
                            handles.push(h);
                        }
                    }
                    for h in handles.drain(..) {
                        pool.put(h).unwrap();
                    }
                }
            });
        });

        group.bench_function("InlineSlots (pull_batch)", |b| {
            b.iter(|| {
                let pool = InlineSlots4::new();
                for _ in 0..ITERS / 2 {
                    // onw batch == Word::BITS == 64 slots == 2 * CAP
                    if let Some(batch) = pool.pull_batch() {
                        pool.put_batch(batch).unwrap();
                    }
                }
            });
        });

        group.bench_function("InlineSlots (pull_exact)", |b| {
            b.iter(|| {
                let pool = InlineSlots4::new();
                for _ in 0..ITERS {
                    if let Some(batch) = pool.pull_exact::<CAP>() {
                        for h in batch {
                            pool.put(h).unwrap();
                        }
                    }
                }
            })
        });

        group.bench_function("WordInlineSlots", |b| {
            b.iter(|| {
                let pool = InlineWordSlots4::new();
                let mut handles = Vec::with_capacity(CAP);
                for _ in 0..ITERS {
                    for _ in 0..CAP {
                        if let Some(h) = pool.pull() {
                            handles.push(h);
                        }
                    }
                    for h in handles.drain(..) {
                        pool.put(h).unwrap();
                    }
                }
            });
        });

        group.bench_function("ArrayQueue", |b| {
            b.iter(|| {
                let pool = ArrayQueuePool::new(CAP);
                let mut handles = Vec::with_capacity(CAP);
                for _ in 0..ITERS {
                    for _ in 0..CAP {
                        if let Some(h) = pool.pull_() {
                            handles.push(h);
                        }
                    }
                    for h in handles.drain(..) {
                        pool.put_(h);
                    }
                }
            });
        });

        group.finish();
    }
}

// =========================================================================
// Wiring
// =========================================================================
#[cfg(all(feature = "alloc", feature = "word-slots"))]
use api_tier_cost::bench_word_granularity;
#[cfg(feature = "alloc")]
use concurrent::{bench_mpmc, bench_mpsc, bench_spsc};
#[cfg(feature = "alloc")]
use single_thread::bench_single_thread;
#[cfg(all(feature = "alloc", feature = "word-slots"))]
use small_capacity::bench_small_capacity_strategies;

#[cfg(feature = "alloc")]
criterion_group!(single_thread_benches, bench_single_thread);

#[cfg(feature = "alloc")]
criterion_group!(concurrent_benches, bench_spsc, bench_mpsc, bench_mpmc);

#[cfg(all(feature = "alloc", feature = "word-slots"))]
criterion_group!(small_capacity_benches, bench_small_capacity_strategies);

#[cfg(all(feature = "alloc", feature = "word-slots"))]
criterion_group!(api_tier_benches, bench_word_granularity);

#[cfg(all(feature = "alloc", not(feature = "word-slots")))]
criterion_main!(single_thread_benches, concurrent_benches,);

#[cfg(all(feature = "alloc", feature = "word-slots"))]
criterion_main!(
    single_thread_benches,
    concurrent_benches,
    small_capacity_benches,
    api_tier_benches
);

#[cfg(not(feature = "alloc"))]
fn foo() {}

#[cfg(not(feature = "alloc"))]
criterion_main!(foo);
