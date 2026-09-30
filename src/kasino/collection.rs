use kasino::{Collection, Signature, WithCapacity};

use crate::{
    SlotPoolMeta,
    bitshard::BitsetStorage,
    core::{BatchedRawSlotPool, RawBatch, RawSlotPool},
};

/// Represents either a batched or single slot operation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MaybeBatched {
    /// A batch of slots returned as a [`RawBatch`].
    Batch(RawBatch),
    /// A single slot index.
    Single(usize),
}

impl MaybeBatched {
    /// Extracts the [`RawBatch`] from a `Batch` variant.
    ///
    /// # Panics
    /// Panics if called on a `Single` variant.
    #[inline]
    pub fn require_batched(self) -> RawBatch {
        let Self::Batch(batch) = self else {
            unreachable!()
        };
        batch
    }

    /// Extracts the single slot index from a `Single` variant.
    ///
    /// # Panics
    /// Panics if called on a `Batch` variant.
    #[inline]
    pub fn require_single(self) -> usize {
        let Self::Single(index) = self else {
            unreachable!()
        };

        index
    }
}

/// Request type for pulling from the storage: either a batch or a single slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PullRequest {
    /// Request a batch of slots.
    BatchRequest,
    /// Request a single slot.
    SingleRequest,
}

/// Signature for the put (offer) operation on the bitset storage.
#[derive(Debug, Default)]
pub struct BitsetStoragePut;

impl Signature for BitsetStoragePut {
    type Error<'input, 'arm>
        = ()
    where
        Self: 'arm;
    type Input<'a> = MaybeBatched;
    type Output<'input, 'arm>
        = ()
    where
        Self: 'arm;
}

/// Signature for the pull (poll) operation on the bitset storage.
#[derive(Debug, Default)]
pub struct BitsetStoragePull;

impl Signature for BitsetStoragePull {
    type Error<'input, 'arm>
        = ()
    where
        Self: 'arm;
    type Input<'a> = PullRequest;
    type Output<'input, 'arm>
        = MaybeBatched
    where
        Self: 'arm;
}

impl<const WORDS: usize> Collection for BitsetStorage<WORDS> {
    type OfferSignature = BitsetStoragePut;
    type PollSignature = BitsetStoragePull;

    #[inline]
    fn offer<'input, 'arm>(
        &'arm self,
        item: <Self::OfferSignature as Signature>::Input<'input>,
    ) -> Result<
        <Self::OfferSignature as Signature>::Output<'input, 'arm>,
        <Self::OfferSignature as Signature>::Error<'input, 'arm>,
    > {
        match item {
            MaybeBatched::Batch(batch) => {
                // SAFETY: batch is a valid RawBatch from this storage
                unsafe { self.put_raw_batch(batch) }.then_some(()).ok_or(())
            }
            MaybeBatched::Single(index) => {
                // SAFETY: index is a valid slot index from this storage
                unsafe { self.put_raw(index) }.then_some(()).ok_or(())
            }
        }
    }

    #[inline]
    fn poll<'input, 'arm>(
        &'arm self,
        input: <Self::PollSignature as Signature>::Input<'input>,
    ) -> Result<
        <Self::PollSignature as Signature>::Output<'input, 'arm>,
        <Self::PollSignature as Signature>::Error<'input, 'arm>,
    > {
        match input {
            PullRequest::BatchRequest => self.pull_raw_batch().map(MaybeBatched::Batch).ok_or(()),
            PullRequest::SingleRequest => self.pull_raw().map(MaybeBatched::Single).ok_or(()),
        }
    }

    fn len(&self) -> usize {
        SlotPoolMeta::len(self)
    }

    fn capacity(&self) -> usize {
        SlotPoolMeta::capacity(self)
    }

    fn is_empty(&self) -> bool {
        SlotPoolMeta::is_empty(self)
    }
}

impl<const N: usize> WithCapacity<N> for BitsetStorage<N> {
    fn with_capacity() -> Self {
        Self::default()
    }
}
