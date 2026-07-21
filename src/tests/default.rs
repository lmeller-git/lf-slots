use crate::{
    RawSlotPool,
    SlotPool,
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
