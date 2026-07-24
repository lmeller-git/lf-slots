use crate::{
    core::ID,
    core_internal::{Batch, RawBatch, SlotHandle},
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
    /// Returns the ID associated with this pool
    fn id(&self) -> ID;

    /// Pull a `SlotHandle` from the storage if it is not empty.
    fn pull(&self) -> Option<SlotHandle> {
        RawSlotPool::pull_raw(self).map(|idx| SlotHandle::new(idx, self.id()))
    }

    /// Put a `SlotHandle` back into the storage to free the associated slot.
    ///
    /// Errs and returns the `SlotHandle`, if the operation is not permitted.
    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        if *index.id() != self.id() {
            return Err(index);
        }

        // SAFETY:
        // we just validated that the index is associated with this pool
        if unsafe { RawSlotPool::put_raw(self, index.as_usize()) } {
            Ok(())
        } else {
            Err(index)
        }
    }

    /// Pull a `Batch` from the storage if it is not empty.
    fn pull_batch(&self) -> Option<Batch> {
        RawSlotPool::pull_raw_batch(self).map(|raw| Batch::new(self.id(), raw))
    }

    /// Put a `Batch` back into the storage to free the associated slots.
    ///
    /// Errs and returns the `Batch` if the operation is not permitted.
    fn put_batch(&self, batch: Batch) -> Result<(), Batch> {
        if *batch.id() != self.id() {
            return Err(batch);
        }

        // SAFETY:
        // we just validated that the batch is associated with this pool
        if unsafe { RawSlotPool::put_raw_batch(self, *batch.raw()) } {
            Ok(())
        } else {
            Err(batch)
        }
    }

    /// Pulls a batch of exactly `N` SlotHandles from the storage, if it contains enough slots.
    fn pull_exact<const N: usize>(&self) -> Option<[SlotHandle; N]> {
        let batch = RawSlotPool::pull_raw_exact(self);
        let id = self.id();
        batch.map(|arr| arr.map(|slot| SlotHandle::new(slot, id.clone())))
    }
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
    /// Pulls a `RawBatch` from the storage if it is not empty.
    fn pull_raw_batch(&self) -> Option<RawBatch>;
    /// Puts back a `RawBatch` into the storage and frees the associated slots.
    ///
    /// returns `true` if the slots were freed.
    ///
    /// # Safety
    /// This method requires that `batch` referring to a valid word in the underlying storage.
    /// Further it requires that `batch` is a `RawBatch` acquired from the same storage, which was not freed beforehand.
    ///
    /// `batch` is a `RawBatch` reutrned by `pull_raw_batch`
    unsafe fn put_raw_batch(&self, batch: RawBatch) -> bool;

    /// Pulls a batch of exactly `N` SlotHandles from the storage, if it contains enough slots.
    fn pull_raw_exact<const N: usize>(&self) -> Option<[usize; N]> {
        if N > self.len() {
            return None;
        }
        let mut batch = core::array::from_fn(|_| core::mem::MaybeUninit::uninit());
        let mut total_count = 0;
        while let Some(pulled_batch) = self.pull_raw_batch()
            && pulled_batch.count() > 0
        {
            let count = pulled_batch.count();
            if count + total_count >= N {
                let (l, r) = pulled_batch.split_at(N - total_count);
                if let Some(r) = r {
                    // SAFETY:
                    // we just got these slots from the pool and will not use them anymore.
                    unsafe { self.put_raw_batch(r) };
                }
                for (to, from) in batch[total_count..N].iter_mut().zip(l) {
                    to.write(from);
                }
                // SAFETY:
                // we populated total_count == N == batch.capacity() slots with valid SlotHandles
                return Some(unsafe {
                    (&batch as *const [core::mem::MaybeUninit<usize>; N])
                        .cast::<[usize; N]>()
                        .read()
                });
            }
            for (to, from) in batch[total_count..total_count + count]
                .iter_mut()
                .zip(pulled_batch)
            {
                to.write(from);
            }
            total_count += count;
        }

        for taken in &batch[..total_count] {
            // SAFETY:
            // we took these slots from the same pool, have not freed them, will not use them and will not free them again.
            unsafe {
                self.put_raw(
                    // SAFETY:
                    // we populated N - total_count slots with valid SlotHandles and didnt free them yet.
                    // we populated the first N - total_count slots in batch.
                    #[allow(unused_unsafe)]
                    unsafe {
                        taken.assume_init_read()
                    },
                )
            };
        }

        None
    }
}
