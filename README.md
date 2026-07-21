[![Codecov](https://codecov.io/github/lmeller-git/lf-slots/coverage.svg?branch=main)](https://codecov.io/gh/lmeller-git/lf-slots)
![CI Test](https://github.com/lmeller-git/lf-slots/actions/workflows/test.yml/badge.svg?branch=main)
![Safety Test](https://github.com/lmeller-git/lf-slots/actions/workflows/safety.yml/badge.svg?branch=main)
![no_std Test](https://github.com/lmeller-git/lf-slots/actions/workflows/nostd.yml/badge.svg?branch=main)


# lf-slots

Non-blocking Lock-free index store.

<!-- cargo-rdme start -->

`lf-slots` provides datastructures for distributing and managing unique slot indices across multiple threads.

All storage types in this repository are safe to use in a concurrent context, strictly lock-free and will never block the calling thread.

### Storage Types

- **InlineSlots**: statically sized stack allocated storage.
- **Slots**: statically sized heap allocated storage.

Due to limitations with current const expr resolution, InlineSlots should be declared with `define_inline_slots` in order to have the correct size and layout.

### Usage

`lf_slots::InlineSlots`:

```rust
use lf_slots::{define_inline_slots, SlotPool, SlotPoolMeta};

define_inline_slots!(SlotPool42, 42);

let pool = SlotPool42::new();

assert_eq!(pool.capacity(), 42);
assert_eq!(pool.len(), 42);

let handle = pool.pull().unwrap();
assert_eq!(pool.len(), 41);
_ = handle.as_usize();
assert!(pool.put(handle).is_ok());
assert!(pool.is_full());
```

`lf_slots::HeapSlots`:

```rust
#[cfg(feature = "alloc")]
fn run() {
 use lf_slots::{Slots, SlotPool,  SlotPoolMeta};

 let pool = Slots::new(42);

 assert_eq!(pool.capacity(), 42);
 assert_eq!(pool.len(), 42);

 let handle = pool.pull().unwrap();
 assert_eq!(pool.len(), 41);
 _ = handle.as_usize();
 assert!(pool.put(handle).is_ok());
 assert!(pool.is_full());
}

#[cfg(feature = "alloc")]
run();
```

### Platform Support

All storage types use 64 bit or 32 bit atomics, depending on platform. Thus only platforms with 32-bit or 64-bit native atomics are supported.
If the feature `atomic-fallback` is used, no native atomics are necessary.

Layout of storage types is determined based on platform architecture to optimize cache line coherence.

### Feature Flags

- `std`: Enables `std` and `alloc` support.
- `alloc`: Enables `alloc` support, allowing usage of some dynamically allocated queues.
- `atomic-fallback`: Uses `portable-atomic` `fallback` feature for atomics if necessary. It is discouraged to use this feature, as `fallback` internally uses locks.
- `default`: []

### Testing
Current testing is based on:

- **Miri** - to validate pointer arithmetic and catch UB.
- **Loom and Shuttle** - to test for race conditions and blocking code.
- **ASan** - to check for memory corruption.

<!-- cargo-rdme end -->
