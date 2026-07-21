//! A lock-free datastructure for distributing indices to slots to multiple subscribers.

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
