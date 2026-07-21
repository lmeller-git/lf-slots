[![Codecov](https://codecov.io/github/lmeller-git/lf-slots/coverage.svg?branch=main)](https://codecov.io/gh/lmeller-git/lf-slots)
![CI Test](https://github.com/lmeller-git/lf-slots/actions/workflows/test.yml/badge.svg?branch=main)
![Safety Test](https://github.com/lmeller-git/lf-slots/actions/workflows/safety.yml/badge.svg?branch=main)
![no_std Test](https://github.com/lmeller-git/lf-slots/actions/workflows/nostd.yml/badge.svg?branch=main)


# lf-slots

Non-blocking Lock-free index store.

<!-- cargo-rdme start -->

`lf-slots` provides datastructures for distributing and managing unique slot indices across multiple threads.

All storage types in this repository are safe to use in a concurrent context, strictly lock-free  and will never block the calling thread.

### Storage Types

- **InlineStorage**: statically sized stack allocated storage.
- **HeapStorage**: statically sized heap allocated storage.

Due to limitations with current const expr resolution, InlineStorage should be declared with `define_inline_store` in order to have the correct size and layout.

### Usage

`lf_slots::InlineStorage`:

```rust
use lf_slots::{define_inline_store, StorageExt, StorageData};

define_inline_store!(Storage42, new_storage42, 42);

let storage = new_storage42();

assert_eq!(storage.capacity(), 42);
assert_eq!(storage.len(), 42);

let handle = storage.pull().unwrap();
assert_eq!(storage.len(), 41);
_ = handle.as_usize();
assert!(storage.put(handle).is_ok());
assert!(storage.is_full());
```

`lf_slots::HeapStorage`:

```rust
#[cfg(feature = "alloc")]
fn run() {
 use lf_slots::{HeapStorage, StorageExt,  StorageData};

 let storage = HeapStorage::new(42);

 assert_eq!(storage.capacity(), 42);
 assert_eq!(storage.len(), 42);

 let handle = storage.pull().unwrap();
 assert_eq!(storage.len(), 41);
 _ = handle.as_usize();
 assert!(storage.put(handle).is_ok());
 assert!(storage.is_full());
}

#[cfg(feature = "alloc")]
run();
```

### Platform Support

All storage types use 64 bit or 32 bit atomics, depending on platform. Thus only platforms with 32-bit or 64-bit native atomics are supported.
If the feature `atomic-fallback` is used, no native atomics are necessary.

Layout of storage types is determined based on platform arhcitecture, to optimize cache line coherence.

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
