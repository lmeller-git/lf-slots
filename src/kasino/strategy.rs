//! Kasino Strategy types for slot pool scheduling.
//!
//! Uses kasino's built-in RoundRobin strategy directly.
//!
//! For pulls: RoundRobin provides fair arm selection, and kasino's default `collect`
//! tries all arms on pull failure.
//!
//! For puts: We bypass kasino entirely and use direct shard access via index
//! since we know the exact shard from the index.

#![allow(clippy::todo, unused_variables, unused_imports, dead_code)]

use kasino::strategy::{RandomAccess, RoundRobin, Strategy};

/// Default strategy selection (cfg-gated like AutoCoherenceProvider).
///
/// - `std` + not test: RoundRobin (has Default stakes, fair scheduling)
/// - `no_std` + not test: RoundRobin (no_std compatible, has Default stakes)
/// - test/loom/shuttle: RandomAccess (deterministic-ish, no_std compatible)
#[cfg(all(feature = "std", not(test), not(loom), not(shuttle)))]
/// Default strategy for standard library targets: RoundRobin provides fair scheduling
/// and has Default stakes required by InlineBandit.
pub type AutoStrategy<const WORDS: usize> = RoundRobin;

#[cfg(all(not(feature = "std"), not(test), not(loom), not(shuttle)))]
/// Default strategy for no_std targets: RoundRobin provides fair scheduling.
pub type AutoStrategy<const WORDS: usize> = RoundRobin;

#[cfg(any(test, loom, shuttle))]
/// Strategy used during testing - RandomAccess provides deterministic-ish behavior.
pub type AutoStrategy<const WORDS: usize> = RandomAccess;
