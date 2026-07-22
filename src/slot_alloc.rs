use crate::core_internal::{Batch, RawBatch, SlotHandle};

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

// TODO: add pull_batch_exact, which pulls a batch ([SlotHandle;N]) of exact size
// can be default impld using pull, put, pull_batch, put_batch and bathc.split_at

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

    fn pull_batch(&self) -> Option<Batch>;

    fn put_batch(&self, batch: Batch) -> Result<(), Batch>;
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

    fn pull_raw_batch(&self) -> Option<RawBatch>;

    unsafe fn put_raw_batch(&self, batch: RawBatch) -> bool;
}
