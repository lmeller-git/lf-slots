[![Codecov](https://codecov.io/github/lmeller-git/lf-slots/coverage.svg?branch=main)](https://codecov.io/gh/lmeller-git/lf-slots)
![CI Test](https://github.com/lmeller-git/lf-slots/actions/workflows/test.yml/badge.svg?branch=main)
![Safety Test](https://github.com/lmeller-git/lf-slots/actions/workflows/safety.yml/badge.svg?branch=main)
![no_std Test](https://github.com/lmeller-git/lf-slots/actions/workflows/nostd.yml/badge.svg?branch=main)


# lf-slots

Non-blocking Lock-free index store.

<!-- cargo-rdme start -->

A lock-free datastructure for distributing indices to slots across multiple subscribers.

All storage types in this repository are safe to use in a concurrent context and will never block the calling thread.

### Storage Types

- **InlineStorage**: statically sized stack allocated storage.
- **HeapStorage**: statically sized heap allocated storage.

Due to limitations with current const expr resolution, InlineStorage should be declared with `define_inline_store` in order to have the correct size and layout.i

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

TODO

### Feature Flags

TODO

### Testing

TODO

<!-- cargo-rdme end -->
