use crate::{
    define_inline_slots,
    tests::stubs::{linearizable, mpmc, mpsc, spsc},
};

define_inline_slots!(Storage2, 2);

#[test]
fn spsc_impl() {
    loom::model(|| {
        let storage = Storage2::new();
        spsc(storage);
    });
}

#[test]
fn mpsc_impl() {
    loom::model(|| {
        let storage = Storage2::new();
        mpsc(storage);
    });
}

#[test]
fn mpmc_impl() {
    loom::model(|| {
        let storage = Storage2::new();
        mpmc(storage);
    });
}

#[test]
fn linearizable_impl() {
    loom::model(|| {
        let storage = Storage2::new();
        linearizable(storage);
    });
}
