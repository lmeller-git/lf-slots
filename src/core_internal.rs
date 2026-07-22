#[cfg(target_has_atomic = "64")]
pub type Word = u64;
#[cfg(target_has_atomic = "64")]
pub(crate) type AtomicWord = crate::sync::atomic::AtomicU64;

#[cfg(not(target_has_atomic = "64"))]
pub type Word = u32;
#[cfg(not(target_has_atomic = "64"))]
pub(crate) type AtomicWord = crate::sync::atomic::AtomicU32;

#[cfg(not(loom))]
#[allow(unused_qualifications)]
pub(crate) const WORD_BYTES: usize = core::mem::size_of::<Word>();
pub(crate) const WORD_BITS: usize = Word::BITS as usize;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ID {
    repr: Word,
}

impl ID {
    pub(crate) fn next() -> Self {
        #[cfg(target_has_atomic = "64")]
        static ID: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
        #[cfg(not(target_has_atomic = "64"))]
        static ID: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);

        Self {
            repr: ID.fetch_add(1, portable_atomic::Ordering::Relaxed),
        }
    }

    pub(crate) fn clone(&self) -> Self {
        Self { repr: self.repr }
    }
}

/// An owned handle for an allocated slot in a storage.
///
/// This handle cannot be cloned or copied, as it should be returned exactly once to the storage which produced it.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct SlotHandle {
    pool_id: ID,
    slot: usize,
}

impl SlotHandle {
    pub(crate) fn new(idx: usize, id: ID) -> Self {
        Self {
            pool_id: id,
            slot: idx,
        }
    }

    pub(crate) fn id(&self) -> &ID {
        &self.pool_id
    }

    pub unsafe fn from_raw(pool_id: ID, slot: usize) -> Self {
        Self { pool_id, slot }
    }

    /// returns the underlying slot index of this handle
    pub fn as_usize(&self) -> usize {
        self.slot
    }
}

impl core::fmt::Display for SlotHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SlotHandle")
            .field("index", &self.slot)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawBatch {
    pub(crate) starting_idx: usize,
    pub(crate) mask: Word,
}

impl RawBatch {
    #[inline]
    pub const fn count(&self) -> usize {
        self.mask.count_ones() as usize
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.mask == 0
    }

    /// Creates an iterator over the slot indices in this batch.
    #[inline]
    pub fn iter(&self) -> RawBatchIter {
        self.into_iter()
    }

    /// Splits this batch into two: `keep` containing the lowest `n` slots,
    /// and `surplus` containing the remaining slots.
    #[inline]
    pub fn split_at(self, n: usize) -> (Self, Self) {
        let total = self.count();
        if n >= total {
            return (
                self,
                RawBatch {
                    starting_idx: self.starting_idx,
                    mask: 0,
                },
            );
        }
        if n == 0 {
            return (
                RawBatch {
                    starting_idx: self.starting_idx,
                    mask: 0,
                },
                self,
            );
        }

        // Clear the lowest set bit `n` times to isolate the surplus mask
        let mut surplus_mask = self.mask;
        for _ in 0..n {
            surplus_mask &= surplus_mask - 1; // x86 BLSR instruction
        }

        let keep_mask = self.mask ^ surplus_mask;

        (
            RawBatch {
                starting_idx: self.starting_idx,
                mask: keep_mask,
            },
            RawBatch {
                starting_idx: self.starting_idx,
                mask: surplus_mask,
            },
        )
    }
}

/// Dedicated iterator for `RawBatch`.
#[derive(Clone, Debug)]
pub struct RawBatchIter {
    batch: RawBatch,
}

impl Iterator for RawBatchIter {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.batch.mask == 0 {
            None
        } else {
            let bit = self.batch.mask.trailing_zeros() as usize;
            self.batch.mask &= self.batch.mask - 1; // Clear lowest set bit (x86 BLSR)
            Some(self.batch.starting_idx + bit)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.batch.count();
        (len, Some(len))
    }
}

impl ExactSizeIterator for RawBatchIter {}

impl IntoIterator for RawBatch {
    type IntoIter = RawBatchIter;
    type Item = usize;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        RawBatchIter { batch: self }
    }
}

pub struct Batch {
    pub(crate) raw: RawBatch,
    pub(crate) id: ID,
}

impl Batch {
    #[inline]
    pub fn count(&self) -> usize {
        self.raw.count()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
}

// Iterator struct that consumes Batch by value
pub struct BatchIntoIter {
    raw: RawBatchIter,
    id: ID,
}

impl IntoIterator for Batch {
    type IntoIter = BatchIntoIter;
    type Item = SlotHandle;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        // Note: batch is dropped here, if Drop where to be implemented in the future in a nont-trivial manner, we would have to handle that here
        BatchIntoIter {
            raw: self.raw.into_iter(),
            id: self.id,
        }
    }
}

impl Iterator for BatchIntoIter {
    type Item = SlotHandle;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.raw.next()?;
        // SAFETY: Unique index extracted from batch, ownership transferred
        Some(unsafe { SlotHandle::from_raw(self.id.clone(), index) })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.raw.size_hint()
    }
}

impl ExactSizeIterator for BatchIntoIter {}
