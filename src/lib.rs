//! `lf-slots` provides data structures for distributing and managing unique slot indices across multiple threads.
//!
//! All storage types in this crate are safe to use in a concurrent context, strictly lock-free, and will never block the calling thread.
//!
//! ## Storage Types
//!
//! - [`InlineSlots`]: statically sized, stack-allocated storage.
//! - [`Slots`]: statically sized, heap-allocated storage.
//!
//! Due to limitations with current `const` expression resolution, [`InlineSlots`] should be declared with [`define_inline_slots!`] in order to have the correct size and layout.
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
//!  use lf_slots::{Slots, SlotPool, SlotPoolMeta};
//!
//!  let pool = Slots::new(42);
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
//! All storage types use 64-bit or 32-bit atomics depending on the platform. Thus, only platforms with 32-bit or 64-bit native atomics are supported by default.
//! If the feature `atomic-fallback` is used, no native atomics are necessary and all target platforms are supported.
//!
//! The memory layout of storage types is determined based on the target platform architecture to optimize cache line usage.
//!
//! ## Performance
//!
//! Under heavy multi-threaded workloads, naive lock-free slot pools can experience throughput drops due to cross-core cache-line invalidation.
//!
//! To mitigate this, this crate provides [`cache_coherence::CoherenceProvider`] strategies in the [`cache_coherence`] module,
//! which aim to reduce cross-core cache invalidation by spreading cross-core memory accesses across the data structure.
//!
//! > **NOTE**:
//! > The throughput of different scheduling strategies depends heavily on your specific workload and thread count.
//! > For maximum performance, custom [`cache_coherence::CoherenceProvider`] implementations may need to be used and should be chosen based on benchmarks for your specific concurrency patterns.
//!
//! ## Feature Flags
//!
//! - `std`: Enables `std` and `alloc` support.
//! - `alloc`: Enables `alloc` support, allowing usage of dynamically allocated slot pools.
//! - `atomic-fallback`: Uses the `portable-atomic` fallback feature if native atomics are missing. It is discouraged to use this feature when performance matters, as fallback atomics internally rely on locks.
//! - `default`: None
//!
//! ## Testing
//!
//! Current testing is based on:
//!
//! - **Miri** - to validate pointer arithmetic and catch undefined behavior.
//! - **Loom and Shuttle** - to test for race conditions and non-blocking invariants.
//! - **ASan** - to check for memory corruption.

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

mod bitshard;
pub mod cache_coherence;
mod core_internal;
mod slot_alloc;
#[macro_use]
mod storage;
mod sync;

pub use core_internal::{Batch, BatchIter, SlotHandle};
pub use slot_alloc::{SlotPool, SlotPoolMeta};
pub use storage::batched;

pub use crate::storage::InlineSlots;
#[cfg(feature = "alloc")]
pub use crate::storage::{Slots, batched::WordSlots};

pub mod core {
    //! Core functionality for the `lf-slots` crate
    pub use crate::{
        bitshard::{shard_count, words_per_shard},
        core_internal::{ID, RawBatch, RawBatchIter, Word},
        slot_alloc::RawSlotPool,
    };
}

pub mod prelude {
    //! reexports common traits implemeneted by `lf-slot` types.
    pub use crate::{SlotPool, SlotPoolMeta};
}

/// Define a type alias for an `InlineSlots<N, { shards(N) }, { words_per_shard(N) }>`.
/// Computes the number of shards needed for a slot pool of size `N` and defines a type alias with correct layout for it.
///
/// Usage:
///
/// ```rust
/// use lf_slots::{define_inline_slots, SlotPool};
///
/// define_inline_slots!(pub(crate) SlotPool1, 1);
///
/// let pool: SlotPool1 = SlotPool1::new();
/// assert!(pool.pull().is_some());
/// ```
#[macro_export]
macro_rules! define_inline_slots {
    ($vis:vis $name:ident, $n:expr) => {
        $vis type $name = $crate::InlineSlots<$n, { $crate::core::shard_count($n) }, { $crate::core::words_per_shard($n) }>;
    };
}

/// Defines a type alias for a `WordPool<InlineSlots<{ N * WordBits }, { shards(N * WordBits) }, { words_per_shard(N * WordBits) }>>`
/// Computes the number of shards needed for a word slot pool of size `N` and defines a type alias with correct layout for it.
///
/// Usage:
///
/// ```rust
/// use lf_slots::{define_inline_wordslots, SlotPool};
///
/// define_inline_wordslots!(pub(crate) SlotPool1, 1);
///
/// let pool: SlotPool1 = SlotPool1::new();
/// assert!(pool.pull().is_some());
/// ```
#[macro_export]
macro_rules! define_inline_wordslots {
    ($vis:vis $name:ident, $n:expr) => {
        $vis type $name = $crate::batched::WordPool<
            $crate::InlineSlots<
                { $n * $crate::core::Word::BITS as usize },
                { $crate::core::shard_count($n * $crate::core::Word::BITS as usize) },
                { $crate::core::words_per_shard($n * $crate::core::Word::BITS as usize) }
            >
        >;
    };
}
