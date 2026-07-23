//! This module contains traits and types to alter the scheduling behaviour of `SlotPools`.
//! `SlotPools` use [`CoherenceProvider`]s to reduce cache-line invalidation due to cross-thread contention.
//!
//! The default `CoherenceProvider` across this crate is [`AutoCoherenceProvider`], which chooses a `CoherenceProvider` based on feature flags.
//!
//! If no or very low thread contention is to be expected OR if the number of shards present in the slot pool are much smaller than the number of threads, [`NoCoherence should be used.
//!
//! Note that it is strongly depended on workload and threading model which coherence model would improve performance and models that are good under some particular workload
//! may reduce performance under another workload. Thus if performance is critical, the correct coherence implementation should be chosen based on benchmarks and performance profiling.
//! In many cases a specialized implementation may also yield better results than the general implementations provided by this crate.

#[cfg(feature = "std")]
use std::hash::{Hash, Hasher};

use crossbeam_utils::CachePadded;
#[cfg(feature = "std")]
use thread_local::ThreadLocal;

use crate::{
    bitshard::BITS_PER_CACHE_LINE,
    sync::atomic::{AtomicUsize, Ordering},
};

/// interface for a type used to improve cacheline coherence under contention
pub trait CoherenceProvider {
    /// returns a discriminant used to inform the slot pool of the identiy of the callign thread.
    fn current_hint(&self) -> usize;
    /// huhu
    fn advance_hint_by(&self, count: usize);
}

/// Does not perfrom any scheduling.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCoherence;

impl CoherenceProvider for NoCoherence {
    fn current_hint(&self) -> usize {
        0
    }

    fn advance_hint_by(&self, _: usize) {}
}

/// per thread round robin
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct ThreadLocalRoundRobin<const STEP: usize = BITS_PER_CACHE_LINE> {
    state: CachePadded<ThreadLocal<core::cell::Cell<(usize, usize)>>>,
}

#[cfg(feature = "std")]
impl<const STEP: usize> ThreadLocalRoundRobin<STEP> {
    /// Constructs a new ThreadLocalRoundRobin
    pub fn new() -> Self {
        Self {
            state: ThreadLocal::new().into(),
        }
    }
}

#[cfg(feature = "std")]
impl<const STEP: usize> CoherenceProvider for ThreadLocalRoundRobin<STEP> {
    #[inline]
    fn current_hint(&self) -> usize {
        self.state
            .get_or(|| {
                let current_thread_id = crate::sync::thread::current().id();
                let mut hasher = std::hash::DefaultHasher::new();
                current_thread_id.hash(&mut hasher);
                let start = hasher.finish();
                core::cell::Cell::new((start as usize, 1))
            })
            .get()
            .0
    }

    #[inline]
    fn advance_hint_by(&self, count: usize) {
        let state = self.state.get_or(|| {
            let current_thread_id = crate::sync::thread::current().id();
            let mut hasher = std::hash::DefaultHasher::new();
            current_thread_id.hash(&mut hasher);
            let start = hasher.finish();
            core::cell::Cell::new((start as usize, 1))
        });

        let (mut hint, mut counter) = state.get();

        counter += count;
        if counter >= STEP {
            let steps = counter / STEP;
            counter %= STEP;
            hint = hint.wrapping_add(steps);
        }
        state.set((hint, counter));
    }
}

/// Sharded Round-Robin provider for `no_std`.
pub struct StripedRoundRobin<const STRIPES: usize = 8, const STEP: usize = BITS_PER_CACHE_LINE> {
    stripes: [CachePadded<(AtomicUsize, AtomicUsize)>; STRIPES],
}

impl<const STRIPES: usize, const STEP: usize> StripedRoundRobin<STRIPES, STEP> {
    /// New StripedRoundRobin
    pub fn new() -> Self {
        Self {
            stripes: core::array::from_fn(|i| (AtomicUsize::new(i), AtomicUsize::new(1)).into()),
        }
    }

    /// Heuristic to select a stripe based on the current stack pointer address.
    fn current_stripe_idx(&self) -> usize {
        let dummy = 0u8;
        let stack_ptr = &dummy as *const u8 as usize;
        let hash = stack_ptr.wrapping_mul(11400714819323198485);
        (hash >> 60) % STRIPES
    }
}

impl<const STRIPES: usize, const STEP: usize> Default for StripedRoundRobin<STRIPES, STEP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const STRIPES: usize, const STEP: usize> CoherenceProvider
    for StripedRoundRobin<STRIPES, STEP>
{
    #[inline]
    fn current_hint(&self) -> usize {
        //TODO: get core id
        let id = self.current_stripe_idx();
        self.stripes[id].0.load(Ordering::Relaxed)
    }

    fn advance_hint_by(&self, count: usize) {
        // TODO get core id
        let id = self.current_stripe_idx();
        let mut counter = self.stripes[id].1.load(Ordering::Relaxed);

        counter += count;
        if counter >= STEP {
            let steps = counter / STEP;
            counter %= STEP;
            _ = self.stripes[id]
                .0
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |hint| {
                    Some(hint.wrapping_add(steps))
                });
        }
        self.stripes[id].1.store(counter, Ordering::Relaxed);
    }
}

/// chooses a good default coherence provider
#[derive(Default)]
pub struct AutoCoherenceProvider {
    #[cfg(any(loom, shuttle))]
    provider: NoCoherence,
    #[cfg(all(not(feature = "std"), not(loom), not(shuttle)))]
    provider: StripedRoundRobin,
    #[cfg(all(feature = "std", not(loom), not(shuttle)))]
    provider: ThreadLocalRoundRobin,
}

impl CoherenceProvider for AutoCoherenceProvider {
    fn current_hint(&self) -> usize {
        self.provider.current_hint()
    }

    fn advance_hint_by(&self, count: usize) {
        self.provider.advance_hint_by(count);
    }
}
