use kasino::{Collection, Signature};

use crate::{
    SlotPoolMeta,
    bitshard::BitsetStorage,
    core::{BatchedRawSlotPool, RawBatch, RawSlotPool},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MaybeBatched {
    Batch(RawBatch),
    Single(usize),
}

impl MaybeBatched {
    #[inline]
    pub fn require_batched(self) -> RawBatch {
        let Self::Batch(batch) = self else {
            unreachable!()
        };
        batch
    }

    #[inline]
    pub fn require_single(self) -> usize {
        let Self::Single(index) = self else {
            unreachable!()
        };

        index
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PullRequest {
    BatchRequest,
    SingleRequest,
}

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
            // SAFETY:
            // we validatedt that this is the correct shard and poll already
            MaybeBatched::Batch(batch) => unsafe { self.put_raw_batch(batch) }.ok_or(()),
            // SAFETY:
            // we validatedt that this is the correct shard and poll already
            MaybeBatched::Single(index) => unsafe { self.put_raw(index) }.ok_or(()),
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
            PullRequest::BatchRequest => self
                .pull_raw_batch()
                .map(|batch| MaybeBatched::Batch(batch))
                .ok_or(()),
            PullRequest::SingleRequest => self
                .pull_raw()
                .map(|index| MaybeBatched::Single(index))
                .ok_or(()),
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
