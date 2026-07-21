use crate::{
    define_inline_slots,
    tests::stubs::{linearizable, mpmc, mpsc, spsc},
};

define_inline_slots!(Storage10, 10);

#[test]
fn spsc_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            spsc(storage);
        },
        100,
        4,
    );
}

#[test]
fn mpsc_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            mpsc(storage);
        },
        100,
        4,
    );
}

#[test]
fn mpmc_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            mpmc(storage);
        },
        100,
        4,
    );
}

#[test]
fn linearizable_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            linearizable(storage);
        },
        100,
        4,
    );
}
