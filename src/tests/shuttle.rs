use crate::{
    InlineSlots,
    tests::stubs::{linearizable, mpmc, mpsc, spsc},
};

#[test]
fn spsc_impl() {
    shuttle::check_pct(
        || {
            let storage: InlineSlots<4> = InlineSlots::new();
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
            let storage: InlineSlots<4> = InlineSlots::new();
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
            let storage: InlineSlots<4> = InlineSlots::new();
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
            let storage: InlineSlots<4> = InlineSlots::new();
            linearizable(storage);
        },
        100,
        4,
    );
}
