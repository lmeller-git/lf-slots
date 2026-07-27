use crate::{
    define_inline_slots,
    tests::stubs::{
        batch_mpmc,
        batch_spsc,
        exact_batch_mpmc,
        linearizable,
        mixed_mpmc,
        mpmc,
        mpsc,
        spsc,
    },
};

define_inline_slots!(Storage2, 2);
define_inline_slots!(Storage10, 10);

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

#[test]
fn batch_spsc_impl() {
    loom::model(|| {
        let storage = Storage2::new();
        batch_spsc(storage);
    });
}

#[test]
fn batch_mpmc_impl() {
    loom::model(|| {
        let storage = Storage2::new();
        batch_mpmc(storage);
    });
}

#[test]
fn exact_batch_mpmc_impl() {
    loom::model(|| {
        let storage = Storage10::new();
        exact_batch_mpmc(storage);
    })
}

#[test]
fn mixed_mpmc_impl() {
    loom::model(|| {
        let storage = Storage2::new();
        mixed_mpmc(storage);
    });
}
