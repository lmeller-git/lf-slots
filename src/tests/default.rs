use crate::{
    RawStorage,
    StorageExt,
    define_inline_store,
    tests::stubs::{len_empty_full, linearizable, mpmc, mpsc, smoke, smoke_long, spsc},
};

define_inline_store!(Storage2, storage2, 2);

define_inline_store!(Storage10, storage10, 10);

define_inline_store!(Storage1000, storage1000, 1000);

#[test]
fn smoke_impl() {
    let storage = storage2();
    smoke(storage);
}

#[test]
fn holds_n() {
    define_inline_store!(Storage42, storage42, 42);

    let storage = storage42();

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
    let storage = storage2();
    len_empty_full(storage);
}

#[test]
fn smoke_long_impl() {
    let storage = storage10();
    smoke_long(storage);
}

#[test]
fn spsc_impl() {
    let storage = storage1000();
    spsc(storage);
}

#[test]
fn mpsc_impl() {
    let storage = storage1000();
    mpsc(storage);
}

#[test]
fn mpmc_impl() {
    let storage = storage1000();
    mpmc(storage);
}

#[test]
fn linearizable_impl() {
    let storage = storage10();
    linearizable(storage);
}

#[cfg(feature = "alloc")]
mod heap {
    use super::*;
    use crate::HeapStorage;

    #[test]
    fn smoke_impl() {
        let storage = HeapStorage::new(2);
        smoke(storage);
    }

    #[test]
    fn holds_n() {
        let storage = HeapStorage::new(42);

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
        let storage = HeapStorage::new(2);
        len_empty_full(storage);
    }

    #[test]
    fn smoke_long_impl() {
        let storage = HeapStorage::new(10);
        smoke_long(storage);
    }

    #[test]
    fn spsc_impl() {
        let storage = HeapStorage::new(1000);
        spsc(storage);
    }

    #[test]
    fn mpsc_impl() {
        let storage = HeapStorage::new(1000);
        mpsc(storage);
    }

    #[test]
    fn mpmc_impl() {
        let storage = HeapStorage::new(1000);
        mpmc(storage);
    }

    #[test]
    fn linearizable_impl() {
        let storage = HeapStorage::new(10);
        linearizable(storage);
    }
}
