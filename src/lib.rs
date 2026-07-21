//! `lf-slots` provides datastructures for distributing and managing unique slot indices across multiple threads.
//!
//! All storage types in this repository are safe to use in a concurrent context, strictly lock-free and will never block the calling thread.
//!
//! ## Storage Types
//!
//! - **InlineSlots**: statically sized stack allocated storage.
//! - **HeapSlots**: statically sized heap allocated storage.
//!
//! Due to limitations with current const expr resolution, InlineSlots should be declared with `define_inline_slots` in order to have the correct size and layout.
//!
//! ## Usage
//!
//! `lf_slots::InlineSlots`:
//!
//! ```rust
//! use lf_slots::{define_inline_slots, SlotPool, SlotPoolMeta};
//!
//! define_inline_slots!(SlotPool42, 42);
//!
//! let pool = SlotPool42::new();
//!
//! assert_eq!(pool.capacity(), 42);
//! assert_eq!(pool.len(), 42);
//!
//! let handle = pool.pull().unwrap();
//! assert_eq!(pool.len(), 41);
//! _ = handle.as_usize();
//! assert!(pool.put(handle).is_ok());
//! assert!(pool.is_full());
//! ```
//!
//! `lf_slots::HeapSlots`:
//!
//! ```rust
//! #[cfg(feature = "alloc")]
//! fn run() {
//!  use lf_slots::{HeapSlots, SlotPool,  SlotPoolMeta};
//!
//!  let pool = HeapSlots::new(42);
//!
//!  assert_eq!(pool.capacity(), 42);
//!  assert_eq!(pool.len(), 42);
//!
//!  let handle = pool.pull().unwrap();
//!  assert_eq!(pool.len(), 41);
//!  _ = handle.as_usize();
//!  assert!(pool.put(handle).is_ok());
//!  assert!(pool.is_full());
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
//! Layout of storage types is determined based on platform architecture to optimize cache line coherence.
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

pub use slot_alloc::{RawSlotPool, SlotHandle, SlotPool, SlotPoolMeta};

#[cfg(feature = "alloc")]
pub use crate::storage::HeapSlots;
pub use crate::storage::InlineSlots;

pub mod core {
    //! core funcitionality of `lf-slots` intended for more fine-grained control than top level exports.
    pub use crate::storage::{full_shard_count, tail_bits};
}

pub mod prelude {
    //! reexports common traits implemeneted by `lf-slot` types.
    pub use crate::{RawSlotPool, SlotPool, SlotPoolMeta};
}

/// Define a type alias for an `InlineSlots<N, { shards(N) }>`.
/// Computes the number of shards needed for a slot pool of size `N` and defines a type alias with correct layout for it.
///
/// Usage:
///
/// ```rust
/// use lf_slots::{define_inline_slots, SlotPool};
///
/// define_inline_slots!(pub(crate) SlotPool0, 0);
///
/// let pool: SlotPool0 = SlotPool0::new();
/// assert!(pool.pull().is_none());
/// ```
#[macro_export]
macro_rules! define_inline_slots {
    ($vis:vis $name:ident, $n:expr) => {
        $vis type $name = $crate::InlineSlots<$n, { $crate::core::full_shard_count($n) }>;
    };
}
