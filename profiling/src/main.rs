#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use std::{env, sync::Arc, thread, time::Instant};

use lf_slots::{HeapStorage, StorageExt};

const CAPACITY: usize = 2048;
const TOTAL_OPS: usize = 20_000_000;
const BATCH_SIZE: usize = 16;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    let mode = env::args()
        .nth(1)
        .unwrap_or_else(|| "isolated-8".to_string());

    println!("Starting profiling run for target: '{mode}'...");
    let start = Instant::now();

    match mode.as_str() {
        "isolated-1" => run_isolated(1),
        "isolated-2" => run_isolated(2),
        "isolated-4" => run_isolated(4),
        "isolated-8" => run_isolated(8),
        other => {
            eprintln!(
                "Unknown target '{other}'. Use: isolated-1, isolated-2, isolated-4, isolated-8"
            );
            return;
        }
    }

    let elapsed = start.elapsed();
    let mops = (TOTAL_OPS as f64 / 1_000_000.0) / elapsed.as_secs_f64();
    println!("Finished in {elapsed:.3?} ({mops:.2} Melem/s)");
}

fn run_isolated(threads: usize) {
    let slots = Arc::new(HeapStorage::<8>::new(CAPACITY));
    let ops_per_thread = TOTAL_OPS / threads;

    let mut handles = Vec::with_capacity(threads);

    for _ in 0..threads {
        let s_clone = slots.clone();
        handles.push(thread::spawn(move || {
            let mut local_batch = Vec::with_capacity(BATCH_SIZE);
            let loops = ops_per_thread / BATCH_SIZE;

            for _ in 0..loops {
                // 1. Pull BATCH_SIZE slots directly from storage
                for _ in 0..BATCH_SIZE {
                    loop {
                        if let Some(handle) = s_clone.pull() {
                            local_batch.push(handle);
                            break;
                        }
                        std::hint::spin_loop();
                    }
                }

                // 2. Put BATCH_SIZE slots back into storage
                for handle in local_batch.drain(..) {
                    let _ = s_clone.put(handle);
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
