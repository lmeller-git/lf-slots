/// The basic underlying storage unit.
///
/// The size of this unit differs by architecture.
#[cfg(target_has_atomic = "64")]
pub type Word = u64;
/// The basic underlying storage unit.
///
/// The size of this unit differs by architecture.
#[cfg(not(target_has_atomic = "64"))]
pub type Word = u32;

#[cfg(target_has_atomic = "64")]
pub(crate) type AtomicWord = crate::sync::atomic::AtomicU64;
#[cfg(not(target_has_atomic = "64"))]
pub(crate) type AtomicWord = crate::sync::atomic::AtomicU32;

#[cfg(not(loom))]
#[allow(unused_qualifications)]
pub(crate) const WORD_BYTES: usize = core::mem::size_of::<Word>();
pub(crate) const WORD_BITS: usize = Word::BITS as usize;

/// The ID associated with a `SlotPool` and the slots handed out by it.
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

    /// Constructs an `ID` from a raw `Word`.
    pub fn from_raw(raw: Word) -> Self {
        Self { repr: raw }
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

    /// Constructs a SlotHandle from an ID and a slot
    ///
    /// # Safety
    /// It is always safe to construct a SlotHandle in this way, however it is NOT safe to return a SlotHandle constructed in this way to a pool.
    /// The Safety requirements are the same as for `RawSlotPool::put_raw`.
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

/// A raw acquired word.
/// A batch may contain between 0 and `Word::BITS` slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawBatch {
    pub(crate) starting_idx: usize,
    pub(crate) mask: Word,
}

impl RawBatch {
    /// Counts the number of acquired slots in this batch
    pub const fn count(&self) -> usize {
        self.mask.count_ones() as usize
    }

    /// Is this batch empty?
    pub const fn is_empty(&self) -> bool {
        self.mask == 0
    }

    /// Splits the batch into two batches of sizes `n`, `self.count() - n`.
    /// If `n` >= `self.count()`, returns `(self, None)`.
    ///
    /// Splits into (high bits, low bits).
    pub fn split_at(self, n: usize) -> (Self, Option<Self>) {
        let total = self.count();
        if n >= total {
            return (self, None);
        }
        if n == 0 {
            return (
                RawBatch {
                    starting_idx: self.starting_idx,
                    mask: 0,
                },
                Some(self),
            );
        }

        let mut surplus_mask = self.mask;
        for _ in 0..n {
            surplus_mask &= surplus_mask - 1;
        }

        (
            RawBatch {
                starting_idx: self.starting_idx,
                mask: self.mask ^ surplus_mask,
            },
            Some(RawBatch {
                starting_idx: self.starting_idx,
                mask: surplus_mask,
            }),
        )
    }
}

/// An Iterator over a `RawBatch`
#[derive(Clone, Debug)]
pub struct RawBatchIter {
    batch: RawBatch,
}

impl Iterator for RawBatchIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.batch.mask == 0 {
            None
        } else {
            let bit = self.batch.mask.trailing_zeros() as usize;
            self.batch.mask &= self.batch.mask - 1;
            Some(self.batch.starting_idx + bit)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.batch.count();
        (len, Some(len))
    }
}

impl ExactSizeIterator for RawBatchIter {}

impl IntoIterator for RawBatch {
    type IntoIter = RawBatchIter;
    type Item = usize;

    fn into_iter(self) -> Self::IntoIter {
        RawBatchIter { batch: self }
    }
}

/// An acquired word.
/// A `Batch` may contain between 0 and `Word::BITS` slots.
pub struct Batch {
    raw: RawBatch,
    id: ID,
}

impl Batch {
    pub(crate) fn new(id: ID, raw: RawBatch) -> Self {
        Self { raw, id }
    }

    pub(crate) fn id(&self) -> &ID {
        &self.id
    }

    pub(crate) fn raw(&self) -> &RawBatch {
        &self.raw
    }

    /// Counts the number of acquired Slots in this batch.
    pub fn count(&self) -> usize {
        self.raw.count()
    }

    /// Is this batch empty?
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// Splits the batch into two batches of sizes `n`, `self.count() - n`.
    /// If `n` >= `self.count()`, returns `(self, None)`.
    ///
    /// Splits into (high bits, low bits).
    pub fn split_at(self, n: usize) -> (Self, Option<Self>) {
        let (r1, r2) = self.raw.split_at(n);
        (
            Batch {
                raw: r1,
                id: self.id.clone(),
            },
            r2.map(|raw| Batch { raw, id: self.id }),
        )
    }
}

/// An iterator over a `Batch`
pub struct BatchIter {
    raw: RawBatchIter,
    id: ID,
}

impl IntoIterator for Batch {
    type IntoIter = BatchIter;
    type Item = SlotHandle;

    fn into_iter(self) -> Self::IntoIter {
        // Note: batch is dropped here, if Drop where to be implemented in the future in a non-trivial manner, we would have to handle that here
        BatchIter {
            raw: self.raw.into_iter(),
            id: self.id,
        }
    }
}

impl Iterator for BatchIter {
    type Item = SlotHandle;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.raw.next()?;
        // SAFETY:
        // BatchIter takes ownership of Batch.
        // RawBatchIter::next takes ownership of a slot index.
        // ID stays the same.
        // The slot is not leaked.
        Some(unsafe { SlotHandle::from_raw(self.id.clone(), index) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.raw.size_hint()
    }
}

impl ExactSizeIterator for BatchIter {}

pub(crate) fn unlikely(v: bool) -> bool {
    if v {
        core::hint::cold_path();
    }
    v
}
