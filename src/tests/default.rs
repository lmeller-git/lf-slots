use crate::{
    SlotPool,
    core::RawSlotPool,
    define_inline_slots,
    tests::stubs::{len_empty_full, linearizable, mpmc, mpsc, smoke, smoke_long, spsc},
};

define_inline_slots!(Storage2, 2);

define_inline_slots!(Storage10, 10);

define_inline_slots!(Storage1000, 1000);

#[test]
fn smoke_impl() {
    let storage = Storage2::new();
    smoke(storage);
}

#[test]
fn holds_n() {
    define_inline_slots!(Storage42, 42);

    let storage = Storage42::new();

    while let Some(idx) = storage.pull_raw() {
        assert!(idx < 42);
    }

    for i in 0..42 {
        // SAFETY:
        // the cap is 43 and the pool is empty
        unsafe { storage.put_raw(i) };
    }

    while let Some(idx) = storage.pull() {
        assert!(idx.as_usize() < 42);
    }
}

#[test]
fn len_impl() {
    let storage = Storage2::new();
    len_empty_full(storage);
}

#[test]
fn smoke_long_impl() {
    let storage = Storage10::new();
    smoke_long(storage);
}

#[test]
fn spsc_impl() {
    let storage = Storage1000::new();
    spsc(storage);
}

#[test]
fn mpsc_impl() {
    let storage = Storage1000::new();
    mpsc(storage);
}

#[test]
fn mpmc_impl() {
    let storage = Storage1000::new();
    mpmc(storage);
}

#[test]
fn linearizable_impl() {
    let storage = Storage10::new();
    linearizable(storage);
}

#[cfg(feature = "alloc")]
mod heap {
    use super::*;
    use crate::Slots;

    #[test]
    fn smoke_impl() {
        let storage = Slots::new(2);
        smoke(storage);
    }

    #[test]
    fn holds_n() {
        let storage = Slots::new(42);

        while let Some(idx) = storage.pull_raw() {
            assert!(idx < 42);
        }

        for i in 0..42 {
            // SAFETY:
            // the cap is 43 and the pool is empty
            unsafe { storage.put_raw(i) };
        }

        while let Some(idx) = storage.pull() {
            assert!(idx.as_usize() < 42);
        }
    }

    #[test]
    fn len_impl() {
        let storage = Slots::new(2);
        len_empty_full(storage);
    }

    #[test]
    fn smoke_long_impl() {
        let storage = Slots::new(10);
        smoke_long(storage);
    }

    #[test]
    fn spsc_impl() {
        let storage = Slots::new(1000);
        spsc(storage);
    }

    #[test]
    fn mpsc_impl() {
        let storage = Slots::new(1000);
        mpsc(storage);
    }

    #[test]
    fn mpmc_impl() {
        let storage = Slots::new(1000);
        mpmc(storage);
    }

    #[test]
    fn linearizable_impl() {
        let storage = Slots::new(10);
        linearizable(storage);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::{SlotPoolMeta, Slots, core::RawBatch};

    /// Helper to expand a `RawBatch` into a `Vec<usize>` of individual slot indices.
    fn expand_raw_batch(batch: RawBatch) -> Vec<usize> {
        let mut indices = Vec::new();
        let mut mask = batch.mask;
        while mask != 0 {
            let bit = mask.trailing_zeros() as usize;
            indices.push(batch.starting_idx + bit);
            mask &= mask - 1; // Clear lowest bit
        }
        indices
    }

    fn create_test_pool() -> impl SlotPool {
        Slots::new(2048)
    }

    #[test]
    fn test_raw_batch_pull_and_put_roundtrip() {
        let pool = create_test_pool();

        // 1. Pull a raw batch
        let batch = pool.pull_raw_batch().expect("Pool should not be empty");
        let indices = expand_raw_batch(batch);

        assert!(!indices.is_empty(), "Batch should contain at least 1 slot");
        assert!(indices.len() <= 64, "Batch mask cannot exceed word size");

        // 2. Put the batch back
        let success = unsafe { pool.put_raw_batch(batch) };
        assert!(success, "put_raw_batch should return true");
    }

    #[test]
    fn test_interleaved_single_and_batch_uniqueness() {
        let pool = create_test_pool();
        let mut allocated = HashSet::new();

        // Pull a single item
        if let Some(idx) = pool.pull_raw() {
            assert!(allocated.insert(idx), "Duplicate slot returned");
        }

        // Pull a batch
        if let Some(batch) = pool.pull_raw_batch() {
            for idx in expand_raw_batch(batch) {
                assert!(
                    allocated.insert(idx),
                    "Batch slot {idx} overlaps with single pull"
                );
            }
        }

        // Pull another single item
        if let Some(idx) = pool.pull_raw() {
            assert!(
                allocated.insert(idx),
                "Single slot {idx} overlaps with batch pull"
            );
        }

        // Return everything back
        for &idx in &allocated {
            unsafe {
                assert!(pool.put_raw(idx));
            }
        }
    }

    #[test]
    fn test_full_exhaustion_via_batches() {
        let pool = create_test_pool();
        let total_capacity = pool.capacity();
        let mut claimed_slots = HashSet::new();

        // Drain the entire pool using pull_raw_batch
        while let Some(batch) = pool.pull_raw_batch() {
            let indices = expand_raw_batch(batch);
            for idx in indices {
                assert!(
                    claimed_slots.insert(idx),
                    "Duplicate slot {idx} detected across batches"
                );
            }
        }

        // Verify pool is now completely empty
        assert_eq!(pool.pull_raw(), None);
        assert_eq!(pool.pull_raw_batch(), None);
        assert_eq!(
            claimed_slots.len(),
            total_capacity,
            "Total claimed slots should equal pool capacity"
        );
    }

    #[test]
    fn test_safe_batch_api_roundtrip() {
        let pool = create_test_pool();

        // Pull safe batch
        let batch = pool.pull_batch().expect("Pool should have available slots");

        // Put safe batch back
        let result = pool.put_batch(batch);
        assert!(result.is_ok(), "put_batch should succeed for valid batch");
    }

    #[test]
    fn test_batch_refill_reuse() {
        let pool = create_test_pool();

        // Drain 1 batch
        let batch_1 = pool.pull_raw_batch().expect("Should pull batch 1");
        let indices_1 = expand_raw_batch(batch_1);

        // Return it immediately
        unsafe {
            pool.put_raw_batch(batch_1);
        }

        // Pull again — should be able to re-claim slots
        let batch_2 = pool
            .pull_raw_batch()
            .expect("Should pull batch 2 after refill");
        let indices_2 = expand_raw_batch(batch_2);

        assert_eq!(
            indices_1.len(),
            indices_2.len(),
            "Refilled batch size should match"
        );
    }
}
