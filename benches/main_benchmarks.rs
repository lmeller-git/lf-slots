use std::{sync::Arc, thread};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use crossbeam_queue::ArrayQueue;
#[cfg(feature = "alloc")]
use lf_slots::{HeapStorage, StorageExt};

const CAPACITY: usize = 2048;
const TOTAL_OPS: usize = 120_000;

#[cfg(feature = "alloc")]
fn bench_spsc(c: &mut Criterion) {
    let mut group = c.benchmark_group("SPSC Throughput");
    group.throughput(Throughput::Elements(TOTAL_OPS as u64));

    group.bench_function("InlineSlots", |b| {
        b.iter(|| {
            let slots = Arc::new(HeapStorage::<8>::new(CAPACITY));
            let queue = Arc::new(ArrayQueue::new(CAPACITY));

            let s_clone = slots.clone();
            let q_clone = queue.clone();
            let producer = thread::spawn(move || {
                for _ in 0..TOTAL_OPS {
                    let mut idx = loop {
                        if let Some(idx) = s_clone.pull() {
                            break idx;
                        }
                        std::hint::spin_loop();
                    };
                    while let Err(idx_) = q_clone.push(idx) {
                        idx = idx_;
                        std::hint::spin_loop();
                    }
                }
            });

            let consumer = thread::spawn(move || {
                for _ in 0..TOTAL_OPS {
                    let idx = loop {
                        if let Some(idx) = queue.pop() {
                            break idx;
                        }
                        std::hint::spin_loop();
                    };
                    _ = slots.put(idx);
                }
            });

            producer.join().unwrap();
            consumer.join().unwrap();
        })
    });
    group.finish();
}

#[cfg(feature = "alloc")]
fn bench_mpsc(c: &mut Criterion) {
    let mut group = c.benchmark_group("MPSC Throughput");
    group.throughput(Throughput::Elements(TOTAL_OPS as u64));

    for num_producers in [2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("InlineSlots", format!("{} producers", num_producers)),
            &num_producers,
            |b, &producers| {
                b.iter(|| {
                    let slots = Arc::new(HeapStorage::<8>::new(CAPACITY));
                    let queue = Arc::new(ArrayQueue::new(CAPACITY));
                    let ops_per_producer = TOTAL_OPS / producers;

                    let mut producer_handles = Vec::new();
                    for _ in 0..producers {
                        let s_clone = slots.clone();
                        let q_clone = queue.clone();
                        producer_handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_producer {
                                let mut idx = loop {
                                    if let Some(idx) = s_clone.pull() {
                                        break idx;
                                    }
                                    std::hint::spin_loop();
                                };
                                while let Err(idx_) = q_clone.push(idx) {
                                    idx = idx_;
                                    std::hint::spin_loop();
                                }
                            }
                        }));
                    }

                    let consumer = thread::spawn(move || {
                        for _ in 0..TOTAL_OPS {
                            let idx = loop {
                                if let Some(idx) = queue.pop() {
                                    break idx;
                                }
                                std::hint::spin_loop();
                            };
                            _ = slots.put(idx);
                        }
                    });

                    for p in producer_handles {
                        p.join().unwrap();
                    }
                    consumer.join().unwrap();
                })
            },
        );
    }
    group.finish();
}

#[cfg(feature = "alloc")]
fn bench_mpmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("MPMC Throughput");
    group.throughput(Throughput::Elements(TOTAL_OPS as u64));

    for thread_pairs in [1, 2, 4] {
        let total_threads = thread_pairs * 2;
        group.bench_with_input(
            BenchmarkId::new("InlineSlots", format!("{} total threads", total_threads)),
            &thread_pairs,
            |b, &pairs| {
                b.iter(|| {
                    let slots = Arc::new(HeapStorage::<8>::new(CAPACITY));
                    let queue = Arc::new(ArrayQueue::new(CAPACITY));
                    let ops_per_thread = TOTAL_OPS / pairs;

                    let mut handles = Vec::new();

                    for _ in 0..pairs {
                        let s_clone = slots.clone();
                        let q_clone = queue.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                let mut idx = loop {
                                    if let Some(idx) = s_clone.pull() {
                                        break idx;
                                    }
                                    std::hint::spin_loop();
                                };
                                while let Err(idx_) = q_clone.push(idx) {
                                    idx = idx_;
                                    std::hint::spin_loop();
                                }
                            }
                        }));
                    }

                    for _ in 0..pairs {
                        let s_clone = slots.clone();
                        let q_clone = queue.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                let idx = loop {
                                    if let Some(idx) = q_clone.pop() {
                                        break idx;
                                    }
                                    std::hint::spin_loop();
                                };
                                _ = s_clone.put(idx);
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );
    }
    group.finish();
}

#[cfg(feature = "alloc")]
criterion_group!(benches, bench_spsc, bench_mpsc, bench_mpmc);
#[cfg(feature = "alloc")]
criterion_main!(benches);

#[cfg(not(feature = "alloc"))]
fn foo() {}

#[cfg(not(feature = "alloc"))]
criterion_main!(foo);
