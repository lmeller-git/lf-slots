use core::fmt::Display;

use crate::sync::atomic::Ordering;

pub trait StorageData {
    fn len(&self) -> usize;
    fn capacity(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }
}

pub trait StorageExt: RawStorage {
    fn pull(&self) -> Option<SlotHandle>;
    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle>;
}

pub trait RawStorage: StorageData {
    fn pull_raw(&self) -> Option<usize>;
    /// # Safety
    /// This function requires that `index` is in bounds of the underlying storage.
    /// Further it requires that `index` is an index to a slot of this storage, which was not freed beforehand.
    ///
    /// `index` is an index returned by `pull_raw`
    unsafe fn put_raw(&self, index: usize) -> bool;
}

pub(crate) fn next_id() -> u64 {
    static ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
    ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct SlotHandle {
    pool_id: u64,
    slot: usize,
}

impl SlotHandle {
    pub(crate) fn new(idx: usize, id: u64) -> Self {
        Self {
            pool_id: id,
            slot: idx,
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.pool_id
    }

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
