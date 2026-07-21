use core::fmt::Display;

use crate::{storage::Word, sync::atomic::Ordering};

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
