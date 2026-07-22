use core::fmt::Display;

use crate::{
    storage::Word,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Metadata of a Storage
pub trait SlotPoolMeta {
    /// The length of a storage.
    ///
    /// In the context of this crate this is the number of free slots
    fn len(&self) -> usize;
    /// The capacity of the storage.
    ///
    /// In the context of this crate this is the maximal number of free slots.
    fn capacity(&self) -> usize;

    /// Is the storage empty?
    ///
    /// In the context of this crate a storage is empty if all slots are allocated.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Is the storage full?
    ///
    /// In the context of this crate a storage is full if all slots are free.
    fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }
}

/// Safe interface for an index storage
///
/// This is a safe wrapper of `RawStorage`.
pub trait SlotPool: RawSlotPool {
    /// Pull a `SlotHandle` from the storage if it is not empty.
    fn pull(&self) -> Option<SlotHandle>;
    /// Put a `SlotHandle` back into the storage to free the associated slot.
    ///
    /// Errs and returns the `SlotHandle`, if the operation is not permitted.
    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle>;
}

/// Raw interface for an index storage.
///
/// Using this trait is unsafe.
/// Underlying implementations may not ensure ABA safety, bound checking or double free safety.
pub trait RawSlotPool: SlotPoolMeta {
    /// Pulls a raw slot index from the storage if it is not empty.
    fn pull_raw(&self) -> Option<usize>;
    /// Puts back a raw slot index into the storage.
    ///
    /// returns `true` if the slot was freed.
    ///
    /// # Safety
    /// This function requires that `index` is in bounds of the underlying storage.
    /// Further it requires that `index` is an index to a slot of this storage, which was not freed beforehand.
    ///
    /// `index` is an index returned by `pull_raw`
    unsafe fn put_raw(&self, index: usize) -> bool;
}

pub(crate) mod coherence {
    #[cfg(feature = "std")]
    use std::hash::{Hash, Hasher};

    use crossbeam_utils::CachePadded;

    use super::*;

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
}

pub(crate) fn next_id() -> Word {
    #[cfg(target_has_atomic = "64")]
    static ID: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    #[cfg(not(target_has_atomic = "64"))]
    static ID: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);

    ID.fetch_add(1, Ordering::Relaxed)
}

/// An owned handle for an allocated slot in a storage.
///
/// This handle cannot be cloned or copied, as it should be returned exactly once to the storage which produced it.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct SlotHandle {
    pool_id: Word,
    slot: usize,
}

impl SlotHandle {
    pub(crate) fn new(idx: usize, id: Word) -> Self {
        Self {
            pool_id: id,
            slot: idx,
        }
    }

    pub(crate) fn id(&self) -> Word {
        self.pool_id
    }

    /// returns the underlying slot index of this handle
    pub fn as_usize(&self) -> usize {
        self.slot
    }
}

impl Display for SlotHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SlotHandle")
            .field("index", &self.slot)
            .finish_non_exhaustive()
    }
}
