// #![deny(missing_docs)]
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
    pub use crate::storage::{
        BitsetStorage,
        ConcatStorage,
        InlineStorage,
        MaskedBitsetStorage,
        full_shard_count,
        tail_bits,
    };
}

#[macro_export]
macro_rules! define_inline_store {
    ($name:ident, $ctor:ident, $n:expr) => {
        pub(crate) type $name =
            $crate::core::InlineStorage<$n, { $crate::core::full_shard_count($n, 512) }>;

        pub(crate) fn $ctor() -> $name {
            $crate::core::InlineStorage::new()
        }
    };
}
