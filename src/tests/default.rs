use crate::{
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
