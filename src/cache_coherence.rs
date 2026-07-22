#[cfg(feature = "std")]
use std::hash::{Hash, Hasher};

use crossbeam_utils::CachePadded;

use crate::sync::atomic::{AtomicUsize, Ordering};

/// interface for a type used to improve cacheline coherence under contention
pub trait CoherenceProvider {
    /// returns a discriminant used to inform the slot pool of the identiy of the callign thread.
    fn current_hint(&self) -> usize;
    /// huhu
    fn advance_hint(&self);
}

/// Does not perfrom any scheduling
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCoherence;

impl CoherenceProvider for NoCoherence {
    fn current_hint(&self) -> usize {
        0
    }

    fn advance_hint(&self) {}
}

/// per thread round robin
#[cfg(feature = "std")]
#[derive(Debug, Default, Clone, Copy)]
pub struct ThreadLocalRoundRobin;

#[cfg(feature = "std")]
std::thread_local! {
    static COUNTER: core::cell::Cell<usize> = {
        let current_thread_id = std::thread::current().id();
        let mut hasher = std::hash::DefaultHasher::new();
        current_thread_id.hash(&mut hasher);
        let h = hasher.finish() as usize;

        core::cell::Cell::new(h)
    };

    static COUNTERCOUNTER: core::cell::Cell<usize> = const {core::cell::Cell::new(0)};
}

#[cfg(feature = "std")]
impl CoherenceProvider for ThreadLocalRoundRobin {
    #[inline]
    fn current_hint(&self) -> usize {
        COUNTER.with(|c| c.get())
    }

    #[inline]
    fn advance_hint(&self) {
        let c_counter = COUNTERCOUNTER.with(|c| {
            let val = c.get();
            c.set(val + 1);
            val
        });
        if c_counter.is_multiple_of(16) {
            COUNTER.with(|c| {
                let val = c.get();
                c.set(val + 1);
            });
        }
    }
}

/// Sharded Round-Robin provider for `no_std`.
pub struct StripedRoundRobin<const STRIPES: usize = 8> {
    stripes: [CachePadded<AtomicUsize>; STRIPES],
}

impl<const STRIPES: usize> StripedRoundRobin<STRIPES> {
    /// New StripedRoundRobin
    pub fn new() -> Self {
        Self {
            stripes: core::array::from_fn(|i| AtomicUsize::new(i).into()),
        }
    }
}

impl<const STRIPES: usize> Default for StripedRoundRobin<STRIPES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const STRIPES: usize> CoherenceProvider for StripedRoundRobin<STRIPES> {
    #[inline]
    fn current_hint(&self) -> usize {
        //TODO: get core id
        self.stripes[0].load(Ordering::Relaxed)
    }

    fn advance_hint(&self) {
        // TODO get core id
        self.stripes[0].fetch_add(1, Ordering::Relaxed);
    }
}
/// chooses a good default coherence provider
#[derive(Default)]
pub struct AutoCoherenceProvider {
    #[cfg(not(feature = "std"))]
    provider: NoCoherence,
    #[cfg(feature = "std")]
    provider: NoCoherence,
}

impl CoherenceProvider for AutoCoherenceProvider {
    fn current_hint(&self) -> usize {
        self.provider.current_hint()
    }

    fn advance_hint(&self) {
        self.provider.advance_hint();
    }
}
