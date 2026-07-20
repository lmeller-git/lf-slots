use crate::{
    define_inline_store,
    tests::stubs::{linearizable, mpmc, mpsc, spsc},
};

define_inline_store!(Storage10, storage10, 10);

#[test]
fn spsc_impl() {
    shuttle::check_pct(
        || {
            let storage = storage10();
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
            let storage = storage10();
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
            let storage = storage10();
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
            let storage = storage10();
            linearizable(storage);
        },
        100,
        4,
    );
}
