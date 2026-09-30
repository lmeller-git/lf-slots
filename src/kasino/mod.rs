//! Kasino-based lock-free slot pool implementation.
//!
//! This module provides an alternative implementation using the kasino framework
//! for optionally linearizable slot pools. The public API is identical to the
//! original implementation.

/// Internal collection types used by the kasino-based pool.
pub mod collection;
/// Pool implementations and traits.
pub mod pool;
/// Scheduling strategies for kasino-based pools.
pub mod strategy;

pub use collection::*;
pub use pool::*;
pub use strategy::*;
