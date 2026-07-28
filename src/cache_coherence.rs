//! This module contains traits and types to alter the scheduling behaviour of `SlotPools`.
//! `SlotPools` use [`CoherenceProvider`]s to reduce cache-line invalidation due to cross-thread contention.
//!
//! The default [`CoherenceProvider`] across this crate is [`AutoCoherenceProvider`], which chooses a [`CoherenceProvider`] based on feature flags.
//!
//! If no or very low thread contention is to be expected OR if the number of shards present in the slot pool are much smaller than the number of threads, [`NoCoherence`] should be used.
//!
//! Note that it is strongly depended on workload and threading model which coherence model would improve performance and models that are good under some particular workload
//! may reduce performance under another workload. Thus, if performance is important, the correct coherence implementation should be chosen based on benchmarks and performance profiling.
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
    /// returns a hint used to inform the slot pool of the identiy of the calling thread and its affinity towards a unit of storage.
    fn current_hint(&self) -> usize;
    /// Advances the hints internal state with some weight `count`. This does not necessarily correspond to an increase of `current_hint`.
    ///
    /// count is measured conceptually (not stricly) in bits and may be interpreted as the weigthed "number of bits dirtied by this operation".
    /// thus, it can be used to weight how "dirty" a unit of storage should be before leaving it.
    fn advance_hint_by(&self, count: usize);
}

/// Does not perform any scheduling.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCoherence;

impl CoherenceProvider for NoCoherence {
    fn current_hint(&self) -> usize {
        0
    }

    fn advance_hint_by(&self, _: usize) {}
}

/// per thread round robin over a ring-buffer.
///
/// Each thread walks a ring-buffer at a speed determined by its actions.
///
/// One step is taken per `STEPS` "dirtied bits".
/// `STEP` may be adjusted to increase or decrease the walking speed.
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

    fn state(&self) -> &core::cell::Cell<(usize, usize)> {
        self.state.get_or(|| {
            let current_thread_id = crate::sync::thread::current().id();
            let mut hasher = std::hash::DefaultHasher::new();
            current_thread_id.hash(&mut hasher);
            let start = hasher.finish();
            core::cell::Cell::new((start as usize, 1))
        })
    }
}

#[cfg(feature = "std")]
impl<const STEP: usize> CoherenceProvider for ThreadLocalRoundRobin<STEP> {
    #[inline]
    fn current_hint(&self) -> usize {
        self.state().get().0
    }

    #[inline]
    fn advance_hint_by(&self, count: usize) {
        let state = self.state();
        let (mut hint, mut counter) = state.get();

        // we advance once per STEP advances.
        // the default STEP is BITS_PER_CACHE_LINE
        counter += count;
        if counter >= STEP {
            let steps = counter / STEP;
            counter %= STEP;
            hint = hint.wrapping_add(steps);
        }
        state.set((hint, counter));
    }
}

/// round robin provider for `no_std`.
///
/// Each thread walks a ring-buffer at a speed determined by its actions.
///
/// `STEP` may be adjusted to increase or decrease the walking speed.
///
/// One step is taken per `STEPS` "dirtied bits".
/// `STRIPES` may be adjusted to reflect the correct number of concurrent callers.
pub struct StripedRoundRobin<const STEP: usize = BITS_PER_CACHE_LINE, const STRIPES: usize = 8> {
    stripes: [CachePadded<(AtomicUsize, AtomicUsize)>; STRIPES],
}

impl<const STEP: usize, const STRIPES: usize> StripedRoundRobin<STEP, STRIPES> {
    /// Constructs a new StripedRoundRobin
    pub fn new() -> Self {
        Self {
            stripes: core::array::from_fn(|i| (AtomicUsize::new(i), AtomicUsize::new(1)).into()),
        }
    }

    /// Heuristic to select a stripe based on the current stack pointer address.
    fn current_stripe_idx(&self) -> usize {
        #[cfg(target_pointer_width = "64")]
        const PHI: usize = 0x9E37_79B9_7F4A_7C15;
        #[cfg(target_pointer_width = "32")]
        const PHI: usize = 0x9E37_79B9;

        let dummy = 0u8;
        let stack_ptr = &dummy as *const u8 as usize;

        let hash = stack_ptr.wrapping_mul(PHI);
        (hash >> (usize::BITS - 4)) % STRIPES
    }
}

impl<const STEP: usize, const STRIPES: usize> Default for StripedRoundRobin<STEP, STRIPES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const STEP: usize, const STRIPES: usize> CoherenceProvider
    for StripedRoundRobin<STEP, STRIPES>
{
    #[inline]
    fn current_hint(&self) -> usize {
        //TODO: get core id to reduce false hint sharing due to hash collision
        let id = self.current_stripe_idx();
        self.stripes[id].0.load(Ordering::Relaxed)
    }

    fn advance_hint_by(&self, count: usize) {
        //TODO: get core id to reduce false hint sharing due to hash collision
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
    #[cfg(any(loom, shuttle, test))]
    provider: NoCoherence,
    #[cfg(all(not(feature = "std"), not(test), not(loom), not(shuttle)))]
    provider: StripedRoundRobin,
    #[cfg(all(feature = "std", not(test), not(loom), not(shuttle)))]
    provider: ThreadLocalRoundRobin,
}

impl AutoCoherenceProvider {
    /// Constructs a new `AutoCoherenceProvider`
    pub fn new() -> Self {
        Self::default()
    }
}

impl CoherenceProvider for AutoCoherenceProvider {
    fn current_hint(&self) -> usize {
        self.provider.current_hint()
    }

    fn advance_hint_by(&self, count: usize) {
        self.provider.advance_hint_by(count);
    }
}

#[cfg(all(test, not(loom), not(shuttle), not(miri)))]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    fn stress_no_panic<P: CoherenceProvider + Sync>(provider: &P, threads: usize, iters: usize) {
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    for i in 0..iters {
                        let _ = provider.current_hint();
                        provider.advance_hint_by(i % 7);
                    }
                });
            }
        });
    }

    #[test]
    fn no_coherence_zero() {
        let p = NoCoherence;
        assert_eq!(p.current_hint(), 0);
        p.advance_hint_by(usize::MAX);
        assert_eq!(p.current_hint(), 0,);
    }

    #[cfg(feature = "std")]
    #[test]
    fn striped_round_robin_stress() {
        stress_no_panic(
            &StripedRoundRobin::<8, BITS_PER_CACHE_LINE>::new(),
            8,
            10_000,
        );
    }

    #[test]
    fn striped_round_robin_hint_advances() {
        let p = StripedRoundRobin::<1, 1>::new();
        for _ in 0..100 {
            let before = p.current_hint();
            p.advance_hint_by(1);
            assert!(p.current_hint() > before);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn thread_local_round_robin_survives_concurrent_use() {
        stress_no_panic(
            &ThreadLocalRoundRobin::<BITS_PER_CACHE_LINE>::new(),
            8,
            10_000,
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn thread_local_round_robin_hint_advances() {
        let p = ThreadLocalRoundRobin::<1>::new();
        for _ in 0..1000 {
            let before = p.current_hint();
            p.advance_hint_by(1);
            assert!(p.current_hint() > before);
        }
    }
}
