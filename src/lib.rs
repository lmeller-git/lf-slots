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
mod storage;
mod sync;

#[cfg(feature = "alloc")]
use crate::storage::HeapStorage;
use crate::storage::{BitsetStorage, ConcatStorage, InlineStorage, MaskedBitsetStorage};
