//! `lf-slots` provides datastructures for distributing and managing unique slot indices across multiple threads.
//!
//! All storage types in this repository are safe to use in a concurrent context, strictly lock-free  and will never block the calling thread.
//!
//! ## Storage Types
//!
//! - **InlineStorage**: statically sized stack allocated storage.
//! - **HeapStorage**: statically sized heap allocated storage.
//!
//! Due to limitations with current const expr resolution, InlineStorage should be declared with `define_inline_store` in order to have the correct size and layout.
//!
//! ## Usage
//!
//! `lf_slots::InlineStorage`:
//!
//! ```rust
//! use lf_slots::{define_inline_store, StorageExt, StorageData};
//!
//! define_inline_store!(Storage42, new_storage42, 42);
//!
//! let storage = new_storage42();
//!
//! assert_eq!(storage.capacity(), 42);
//! assert_eq!(storage.len(), 42);
//!
//! let handle = storage.pull().unwrap();
//! assert_eq!(storage.len(), 41);
//! _ = handle.as_usize();
//! assert!(storage.put(handle).is_ok());
//! assert!(storage.is_full());
//! ```
//!
//! `lf_slots::HeapStorage`:
//!
//! ```rust
//! #[cfg(feature = "alloc")]
//! fn run() {
//!  use lf_slots::{HeapStorage, StorageExt,  StorageData};
//!
//!  let storage = HeapStorage::new(42);
//!
//!  assert_eq!(storage.capacity(), 42);
//!  assert_eq!(storage.len(), 42);
//!
//!  let handle = storage.pull().unwrap();
//!  assert_eq!(storage.len(), 41);
//!  _ = handle.as_usize();
//!  assert!(storage.put(handle).is_ok());
//!  assert!(storage.is_full());
//! }
//!
//! #[cfg(feature = "alloc")]
//! run();
//! ```
//!
//! ## Platform Support
//!
//! All storage types use 64 bit or 32 bit atomics, depending on platform. Thus only platforms with 32-bit or 64-bit native atomics are supported.
//! If the feature `atomic-fallback` is used, no native atomics are necessary.
//!
//! Layout of storage types is determined based on platform arhcitecture, to optimize cache line coherence.
//!
//! ## Feature Flags
//!
//! - `std`: Enables `std` and `alloc` support.
//! - `alloc`: Enables `alloc` support, allowing usage of some dynamically allocated queues.
//! - `atomic-fallback`: Uses `portable-atomic` `fallback` feature for atomics if necessary. It is discouraged to use this feature, as `fallback` internally uses locks.
//! - `default`: []
//!
//! ## Testing
//! Current testing is based on:
//!
//! - **Miri** - to validate pointer arithmetic and catch UB.
//! - **Loom and Shuttle** - to test for race conditions and blocking code.
//! - **ASan** - to check for memory corruption.
//!

#![deny(missing_docs)]
#![deny(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)]
#![warn(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(test, feature = "std"))]
mod tests;

mod slot_alloc;
#[macro_use]
mod storage;
mod sync;

pub use slot_alloc::{RawStorage, SlotHandle, StorageData, StorageExt};

#[cfg(feature = "alloc")]
pub use crate::storage::HeapStorage;

pub mod core {
    //! core funcitionality of `lf-slots` intended for more fine-grained control than top level exports.
    pub use crate::storage::{InlineStorage, full_shard_count, tail_bits};
}

/// Define a type alias for an `InlineStorage<N, { shards(N) }>`.
/// Computes the number of shards needed for a storage size `N` and defines a type alias and constructor function for it.
///
/// Usage:
///
/// ```rust
/// use lf_slots::{define_inline_store, StorageExt};
///
/// define_inline_store!(Storage0, storage0, 0);
///
/// let storage: Storage0 = storage0();
/// assert!(storage.pull().is_none());
/// ```
#[macro_export]
macro_rules! define_inline_store {
    ($name:ident, $ctor:ident, $n:expr) => {
        pub(crate) type $name =
            $crate::core::InlineStorage<$n, { $crate::core::full_shard_count($n) }>;

        pub(crate) fn $ctor() -> $name {
            $crate::core::InlineStorage::new()
        }
    };
}
