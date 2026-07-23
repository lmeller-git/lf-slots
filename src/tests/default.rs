use crate::{
    SlotPool,
    SlotPoolMeta,
    cache_coherence::NoCoherence,
    core::RawSlotPool,
    define_inline_slots,
    tests::stubs::{
        batch_mpmc,
        batch_smoke,
        batch_spsc,
        len_empty_full,
        linearizable,
        mixed_mpmc,
        mpmc,
        mpsc,
        smoke,
        smoke_long,
        spsc,
    },
};

define_inline_slots!(Storage2, 2);

define_inline_slots!(Storage10, 10);

define_inline_slots!(Storage2000, 2000);

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
fn order() {
    let storage = Storage2000::with_coherence_provider::<NoCoherence>();
    for i in 0..storage.capacity() {
        assert_eq!(i, storage.pull_raw().unwrap());
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
    let storage = Storage2000::new();
    spsc(storage);
}

#[test]
fn mpsc_impl() {
    let storage = Storage2000::new();
    mpsc(storage);
}

#[test]
fn mpmc_impl() {
    let storage = Storage2000::new();
    mpmc(storage);
}

#[test]
fn linearizable_impl() {
    let storage = Storage10::new();
    linearizable(storage);
}

#[test]
fn smoke_batch_impl() {
    let storage = Storage2000::new();
    batch_smoke(storage);
}

#[test]
fn batch_spsc_impl() {
    let storage = Storage2000::new();
    batch_spsc(storage);
}

#[test]
fn batch_mpmc_impl() {
    let storage = Storage2000::new();
    batch_mpmc(storage);
}

#[test]
fn mixed_mpmc_impl() {
    let storage = Storage2000::new();
    mixed_mpmc(storage);
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
        let storage = Slots::new(2000);
        spsc(storage);
    }

    #[test]
    fn mpsc_impl() {
        let storage = Slots::new(2000);
        mpsc(storage);
    }

    #[test]
    fn mpmc_impl() {
        let storage = Slots::new(2000);
        mpmc(storage);
    }

    #[test]
    fn linearizable_impl() {
        let storage = Slots::new(10);
        linearizable(storage);
    }

    #[test]
    fn smoke_batch_impl() {
        let storage = Slots::new(2000);
        batch_smoke(storage);
    }

    #[test]
    fn batch_spsc_impl() {
        let storage = Slots::new(2000);
        batch_spsc(storage);
    }

    #[test]
    fn batch_mpmc_impl() {
        let storage = Slots::new(2000);
        batch_mpmc(storage);
    }

    #[test]
    fn mixed_mpmc_impl() {
        let storage = Slots::new(2000);
        mixed_mpmc(storage);
    }
}

#[cfg(test)]
mod batch_tests {
    use std::collections::HashSet;

    use super::*;
    use crate::{SlotPoolMeta, Slots};

    fn create_test_pool() -> impl SlotPool {
        Slots::new(2048)
    }

    #[test]
    fn single_batch_single() {
        let pool = create_test_pool();
        let mut allocated = HashSet::new();

        // Pull a single item
        if let Some(idx) = pool.pull_raw() {
            assert!(allocated.insert(idx), "Duplicate slot returned");
        }

        // Pull a batch
        if let Some(batch) = pool.pull_raw_batch() {
            for idx in batch {
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
            // SAFETY:
            // we got this batch from the same pool
            unsafe {
                assert!(pool.put_raw(idx));
            }
        }
    }

    #[test]
    fn batch_exhaustion() {
        let pool = create_test_pool();
        let total_capacity = pool.capacity();
        let mut claimed_slots = HashSet::new();

        // Drain the entire pool using pull_raw_batch
        while let Some(batch) = pool.pull_raw_batch() {
            for idx in batch {
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
    fn pull_exact() {
        let pool = create_test_pool();

        let handles = pool.pull_exact::<16>().unwrap();
        assert_eq!(handles.len(), 16);
        assert_eq!(pool.len(), pool.capacity() - 16);
        for handle in handles {
            pool.put(handle).unwrap();
        }
        assert!(pool.is_full());
        assert_eq!(pool.pull_exact::<1>().unwrap().len(), 1);
    }

    #[test]
    fn pool_exact_large() {
        let pool = Slots::new(128);

        assert!(pool.pull_exact::<256>().is_none());

        let batch_80 = pool.pull_exact::<80>().unwrap();
        assert_eq!(batch_80.len(), 80);

        // Verify the remaining 48 slots are still available and were not lost
        let batch_48 = pool.pull_exact::<48>();
        assert!(
            batch_48.is_some(),
            "Leftovers from previous batch split should still be in pool"
        );

        // Pool should now be completely empty
        assert!(
            pool.pull_exact::<1>().is_none(),
            "Pool should be empty after taking 16 + 48"
        );
        assert!(pool.is_empty());
    }
}
