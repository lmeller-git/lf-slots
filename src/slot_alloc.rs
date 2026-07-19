use crate::sync::atomic::{AtomicU64, Ordering};

pub trait StorageExt {
    fn pull(&self) -> Option<SlotHandle>;
    fn put(&self, index: SlotHandle) -> bool;
}

pub trait RawStorage {
    fn pull_raw(&self) -> Option<usize>;
    unsafe fn put_raw(&self, index: usize) -> bool;
    fn is_empty(&self) -> bool;
    fn is_full(&self) -> bool;
    fn len(&self) -> usize;
    fn capacity(&self) -> usize;
}

pub(crate) fn next_id() -> u64 {
    static ID: AtomicU64 = AtomicU64::new(0);
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
