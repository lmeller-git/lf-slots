use crate::{
    InlineSlots,
    tests::stubs::{len_empty_full, linearizable, mpmc, mpsc, smoke, smoke_long, spsc},
};

#[test]
fn smoke_impl() {
    let storage: InlineSlots<4> = InlineSlots::new();
    smoke(storage);
}

#[test]
fn len_impl() {
    let storage: InlineSlots<2> = InlineSlots::new();
    len_empty_full(storage);
}

#[test]
fn smoke_long_impl() {
    let storage: InlineSlots<10> = InlineSlots::new();
    smoke_long(storage);
}

#[test]
fn spsc_impl() {
    let storage: InlineSlots<4> = InlineSlots::new();
    spsc(storage);
}

#[test]
fn mpsc_impl() {
    let storage: InlineSlots<4> = InlineSlots::new();
    mpsc(storage);
}

#[test]
fn mpmc_impl() {
    let storage: InlineSlots<4> = InlineSlots::new();
    mpmc(storage);
}

#[test]
fn linearizable_impl() {
    let storage: InlineSlots<4> = InlineSlots::new();
    linearizable(storage);
}
