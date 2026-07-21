use crate::{
    RawStorage,
    StorageExt,
    define_inline_store,
    tests::stubs::{len_empty_full, linearizable, mpmc, mpsc, smoke, smoke_long, spsc},
};

define_inline_store!(Storage2, storage2, 2);

define_inline_store!(Storage10, storage10, 10);

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
    let storage = storage10();
    spsc(storage);
}

#[test]
fn mpsc_impl() {
    let storage = storage10();
    mpsc(storage);
}

#[test]
fn mpmc_impl() {
    let storage = storage10();
    mpmc(storage);
}

#[test]
fn linearizable_impl() {
    let storage = storage10();
    linearizable(storage);
}
