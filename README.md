[![Codecov](https://codecov.io/github/lmeller-git/lf-slots/coverage.svg?branch=main)](https://codecov.io/gh/lmeller-git/lf-slots)
![CI Test](https://github.com/lmeller-git/lf-slots/actions/workflows/test.yml/badge.svg?branch=main)
![Safety Test](https://github.com/lmeller-git/lf-slots/actions/workflows/safety.yml/badge.svg?branch=main)
![no_std Test](https://github.com/lmeller-git/lf-slots/actions/workflows/nostd.yml/badge.svg?branch=main)


# lf-slots

Non-blocking Lock-free index store.

<!-- cargo-rdme start -->

`lf-slots` provides data structures for distributing and managing unique slot indices across multiple threads.

All storage types in this crate are safe to use in a concurrent context, strictly lock-free, and will never block the calling thread.

### Storage Types

- [`InlineSlots`](https://docs.rs/lf-slots/latest/lf_slots/storage/struct.InlineSlots.html): statically sized, stack-allocated storage.
- [`Slots`](https://docs.rs/lf-slots/latest/lf_slots/storage/struct.Slots.html): statically sized, heap-allocated storage.

Due to limitations with current `const` expression resolution, [`InlineSlots`](https://docs.rs/lf-slots/latest/lf_slots/storage/struct.InlineSlots.html) should be declared with [`define_inline_slots!`](https://docs.rs/lf-slots/latest/lf_slots/macro.define_inline_slots.html) in order to have the correct size and layout.

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
 use lf_slots::{Slots, SlotPool, SlotPoolMeta};

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

All storage types use 64-bit or 32-bit atomics depending on the platform. Thus, only platforms with 32-bit or 64-bit native atomics are supported by default.
If the feature `atomic-fallback` is used, no native atomics are necessary and all target platforms are supported.

The memory layout of storage types is determined based on the target platform architecture to optimize cache line usage.

### Performance

Under heavy multi-threaded workloads, naive lock-free slot pools can experience throughput drops due to cross-core cache-line invalidation.

To mitigate this, this crate provides [`cache_coherence::CoherenceProvider`](https://docs.rs/lf-slots/latest/lf_slots/cache_coherence/trait.CoherenceProvider.html) strategies in the [`cache_coherence`](https://docs.rs/lf-slots/latest/lf_slots/cache_coherence/) module,
which aim to reduce cross-core cache invalidation by spreading cross-core memory accesses across the data structure.

> **NOTE**:
> The throughput of different scheduling strategies depends heavily on your specific workload and thread count.
> For maximum performance, custom [`cache_coherence::CoherenceProvider`](https://docs.rs/lf-slots/latest/lf_slots/cache_coherence/trait.CoherenceProvider.html) implementations may need to be used and should be chosen based on benchmarks for your specific concurrency patterns.

### Feature Flags

- `std`: Enables `std` and `alloc` support.
- `alloc`: Enables `alloc` support, allowing usage of dynamically allocated slot pools.
- `atomic-fallback`: Uses the `portable-atomic` fallback feature if native atomics are missing. It is discouraged to use this feature when performance matters, as fallback atomics internally rely on locks.
- `default`: None

### Testing

Current testing is based on:

- **Miri** - to validate pointer arithmetic and catch undefined behavior.
- **Loom and Shuttle** - to test for race conditions and non-blocking invariants.
- **ASan** - to check for memory corruption.

<!-- cargo-rdme end -->
