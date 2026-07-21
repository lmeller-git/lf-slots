use crate::{
    define_inline_store,
    tests::stubs::{linearizable, mpmc, mpsc, spsc},
};

define_inline_store!(Storage2, storage2, 2);

#[test]
fn spsc_impl() {
    loom::model(|| {
        let storage = storage2();
        spsc(storage);
    });
}

#[test]
fn mpsc_impl() {
    loom::model(|| {
        let storage = storage2();
        mpsc(storage);
    });
}

#[test]
fn mpmc_impl() {
    loom::model(|| {
        let storage = storage2();
        mpmc(storage);
    });
}

#[test]
fn linearizable_impl() {
    loom::model(|| {
        let storage = storage2();
        linearizable(storage);
    });
}
