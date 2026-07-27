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

const RETRIES: usize = 100;
const DEPTH: usize = 4;

define_inline_slots!(Storage10, 10);

#[test]
fn spsc_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            spsc(storage);
        },
        RETRIES,
        DEPTH,
    );
}

#[test]
fn mpsc_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            mpsc(storage);
        },
        RETRIES,
        DEPTH,
    );
}

#[test]
fn mpmc_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            mpmc(storage);
        },
        RETRIES,
        DEPTH,
    );
}

#[test]
fn linearizable_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            linearizable(storage);
        },
        RETRIES,
        DEPTH,
    );
}

#[test]
fn batch_spsc_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            batch_spsc(storage);
        },
        RETRIES,
        DEPTH,
    );
}

#[test]
fn batch_mpmc_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            batch_mpmc(storage);
        },
        RETRIES,
        DEPTH,
    );
}

#[test]
fn exact_test_mpmc() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            exact_batch_mpmc(storage);
        },
        RETRIES,
        DEPTH,
    )
}

#[test]
fn mixed_mpmc_impl() {
    shuttle::check_pct(
        || {
            let storage = Storage10::new();
            mixed_mpmc(storage);
        },
        RETRIES,
        DEPTH,
    );
}
