//! Kasino-based lock-free slot pool implementation.
//!
//! This module provides an alternative implementation using the kasino framework
//! for optionally linearizable slot pools. The public API is identical to the
//! original implementation.

pub mod collection;
pub mod pool;
pub mod strategy;

#[cfg(feature = "word-slots")]
/// Batched/word-slot pool implementations using kasino.
pub mod batched {
    pub use super::pool::batched::*;
}

pub use collection::{
    BatchView,
    OfferInput,
    OfferOutput,
    PollInput,
    PollOutput,
    ShardCollection,
    SingleView,
};
pub use pool::InlineSlots;
#[cfg(feature = "alloc")]
pub use pool::Slots;
#[cfg(feature = "word-slots")]
pub use pool::batched::KasinoWordPool;
#[cfg(all(feature = "word-slots", feature = "alloc"))]
pub use pool::batched::KasinoWordSlots;
pub use strategy::AutoStrategy;
