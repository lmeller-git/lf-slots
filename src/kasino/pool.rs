use kasino::{Collection, InlineBandit, InlineBanditHandle, strategy::Strategy};

use crate::{
    bitshard::BitsetStorage,
    core::WORDS_PER_CACHE_LINE,
    core_internal::{Batch, ID, RawBatch, SlotHandle},
    kasino::collection::{MaybeBatched, PullRequest},
};

/// Metadata of a Storage
pub trait OwnedSlotPoolMeta {
    /// The length of a storage.
    ///
    /// In the context of this crate this is the number of free slots
    fn len(&mut self) -> usize;
    /// The capacity of the storage.
    ///
    /// In the context of this crate this is the maximal number of free slots.
    fn capacity(&mut self) -> usize;

    /// Is the storage empty?
    ///
    /// In the context of this crate a storage is empty if all slots are allocated.
    #[inline]
    fn is_empty(&mut self) -> bool {
        self.len() == 0
    }

    /// Is the storage full?
    ///
    /// In the context of this crate a storage is full if all slots are free.
    #[inline]
    fn is_full(&mut self) -> bool {
        self.len() == self.capacity()
    }
}

/// Safe interface for an index storage
///
/// This is a safe wrapper of `RawStorage`.
pub trait OwnedSlotPool: RawOwnedSlotPool {
    /// Returns the ID associated with this pool
    fn id(&mut self) -> ID;

    /// Pull a `SlotHandle` from the storage if it is not empty.
    #[inline]
    fn pull(&mut self) -> Option<SlotHandle> {
        RawOwnedSlotPool::pull_raw(self).map(|idx| SlotHandle::new(idx, self.id()))
    }

    /// Put a `SlotHandle` back into the storage to free the associated slot.
    ///
    /// Errs and returns the `SlotHandle`, if the operation is not permitted.
    #[inline]
    fn put(&mut self, index: SlotHandle) -> Result<(), SlotHandle> {
        if *index.id() != self.id() {
            return Err(index);
        }

        // SAFETY:
        // we just validated that the index is associated with this pool
        if unsafe { RawOwnedSlotPool::put_raw(self, index.as_usize()) } {
            Ok(())
        } else {
            Err(index)
        }
    }
}

/// Raw interface for an index storage.
///
/// Using this trait is unsafe.
/// Underlying implementations may not ensure ABA safety, bound checking or double free safety.
pub trait RawOwnedSlotPool: OwnedSlotPoolMeta {
    /// Pulls a raw slot index from the storage if it is not empty.
    fn pull_raw(&mut self) -> Option<usize>;
    /// Puts back a raw slot index into the storage.
    ///
    /// returns `true` if the slot was freed.
    ///
    /// # Safety
    /// This function requires that `index` is in bounds of the underlying storage.
    /// Further it requires that `index` is an index to a slot of this storage, which was not freed beforehand.
    ///
    /// `index` is an index returned by `pull_raw`
    unsafe fn put_raw(&mut self, index: usize) -> bool;
}

/// A RawOwnedSlotPool that supports batched operations
pub trait BatchedRawOwnedSlotPool: RawOwnedSlotPool {
    /// Pulls a `RawBatch` from the storage if it is not empty.
    /// the pulled batch is non-empty.
    fn pull_raw_batch(&mut self) -> Option<RawBatch>;
    /// Puts back a `RawBatch` into the storage and frees the associated slots.
    ///
    /// returns `true` if the slots were freed.
    ///
    /// # Safety
    /// This method requires that `batch` referring to a valid word in the underlying storage.
    /// Further it requires that `batch` is a `RawBatch` acquired from the same storage, which was not freed beforehand.
    ///
    /// `batch` is a `RawBatch` reutrned by `pull_raw_batch`
    unsafe fn put_raw_batch(&mut self, batch: RawBatch) -> bool;

    /// Pulls a batch of exactly `N` slots from the storage, if it contains enough slots.
    ///
    /// This method acquires slots eagerly, even if not enough slots may be available at the moment.
    /// If the capacity of the pool is roughly equal to `N` and the likelihood of the pool not holding enough slots for pull_raw_exact, prefer checking [`OwnedSlotPoolMeta::len`] first:
    /// ```ignore
    /// if pool.len() >= N &&
    ///     let Some(batch) = pool.pull_raw_exact::<N>()
    /// {
    ///    // ...
    /// }
    /// ```
    fn pull_raw_exact<const N: usize>(&mut self) -> Option<[usize; N]> {
        if N > self.capacity() {
            return None;
        }
        let mut batch = core::array::from_fn(|_| core::mem::MaybeUninit::uninit());
        let mut total_count = 0;
        while let Some(pulled_batch) = self.pull_raw_batch() {
            debug_assert!(pulled_batch.count() > 0);
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

/// A OwnedSlotPool that supportes batched operations
pub trait BatchedOwnedSlotPool: BatchedRawOwnedSlotPool + OwnedSlotPool {
    /// Pull a `Batch` from the storage if it is not empty.
    /// The pulled batch is non-empty.
    #[inline]
    fn pull_batch(&mut self) -> Option<Batch> {
        BatchedRawOwnedSlotPool::pull_raw_batch(self).map(|raw| Batch::new(self.id(), raw))
    }

    /// Put a `Batch` back into the storage to free the associated slots.
    ///
    /// Errs and returns the `Batch` if the operation is not permitted.
    #[inline]
    fn put_batch(&mut self, batch: Batch) -> Result<(), Batch> {
        if *batch.id() != self.id() {
            return Err(batch);
        }

        // SAFETY:
        // we just validated that the batch is associated with this pool
        if unsafe { BatchedRawOwnedSlotPool::put_raw_batch(self, *batch.raw()) } {
            Ok(())
        } else {
            Err(batch)
        }
    }

    /// Pulls a batch of exactly `N` SlotHandles from the storage, if it contains enough slots.
    ///
    /// This method acquires slots eagerly, even if not enough slots may be available at the moment.
    /// If the capacity of the pool is roughly equal to `N` and the likelihood of the pool not holding enough slots for pull_exact, prefer checking [`OwnedSlotPoolMeta::len`] first:
    /// ```ignore
    /// if pool.len() >= N &&
    ///     let Some(batch) = pool.pull_exact::<N>()
    /// {
    ///    // ...
    /// }
    /// ```
    #[inline]
    fn pull_exact<const N: usize>(&mut self) -> Option<[SlotHandle; N]> {
        let batch = BatchedRawOwnedSlotPool::pull_raw_exact(self);
        let id = self.id();
        batch.map(|arr| arr.map(|slot| SlotHandle::new(slot, id)))
    }
}

/// A wrapper strategy that adapts a kasino [`Strategy`] for use with the index pool.
///
/// This strategy delegates to an inner scheduler for gambler management while
/// the offer/poll arm selection is handled by the kasino framework directly
/// (these methods are not used in the current implementation).
#[derive(Default)]
pub struct IndexPoolStrategy<S> {
    scheduler: S,
}

impl<S: Strategy<Q>, Q: Collection> Strategy<Q> for IndexPoolStrategy<S> {
    type Gambler = S::Gambler;

    fn choose_offer_arm(
        &self,
        _state: &impl kasino::storage::StorageBackend<
            <Self::Gambler as kasino::strategy::Hooked>::Stake,
        >,
        _gambler: &mut Self::Gambler,
    ) -> usize {
        // Not used: puts use direct shard access via index.
        unreachable!("offer arm selection not used")
    }

    fn choose_poll_arm(
        &self,
        _state: &impl kasino::storage::StorageBackend<
            <Self::Gambler as kasino::strategy::Hooked>::Stake,
        >,
        _gambler: &mut Self::Gambler,
    ) -> usize {
        // Not used: kasino's default `collect` tries all arms on pull failure.
        unreachable!("poll arm selection not used")
    }

    fn fork_gambler(&self, parent: &Self::Gambler) -> Self::Gambler {
        self.scheduler.fork_gambler(parent)
    }

    fn create_gambler(&self) -> Self::Gambler {
        self.scheduler.create_gambler()
    }
}

/// An inline kasino-based slot pool with fixed capacity.
///
/// This pool uses the kasino framework for lock-free synchronization with
/// optional linearizability guarantees. The capacity is determined by the
/// `N`, `SHARDS`, and `WORDS_PER_SHARD` const generics.
///
/// # Type Parameters
/// - `S`: The scheduling strategy (must implement [`Strategy`] for the bitset storage).
/// - `N`: Total number of slots in the pool.
/// - `SHARDS`: Number of shards for contention reduction.
/// - `WORDS_PER_SHARD`: Number of words per shard (default: [`WORDS_PER_CACHE_LINE`]).
#[allow(private_bounds)]
#[expect(dead_code, reason = "fields used via kasino's InlineBandit internals")]
pub struct InlineKasinoSlotPool<
    S: Strategy<BitsetStorage<WORDS_PER_SHARD>>,
    const N: usize,
    const SHARDS: usize,
    const WORDS_PER_SHARD: usize = WORDS_PER_CACHE_LINE,
> {
    bandit:
        InlineBandit<BitsetStorage<WORDS_PER_SHARD>, IndexPoolStrategy<S>, SHARDS, WORDS_PER_SHARD>,
    id: ID,
}

impl<
    S: Strategy<BitsetStorage<WORDS_PER_SHARD>>,
    const N: usize,
    const SHARDS: usize,
    const WORDS_PER_SHARD: usize,
> InlineKasinoSlotPool<S, N, SHARDS, WORDS_PER_SHARD>
where
    S: Default,
    kasino::strategy::StrategyStakes<S, BitsetStorage<WORDS_PER_SHARD>>: Default,
{
    /// Creates a new [`InlineKasinoSlotPool`] with all slots initially free.
    pub fn new() -> Self {
        let bandit = InlineBandit::new();
        Self {
            bandit,
            id: ID::next(),
        }
    }
}

impl<
    S: Strategy<BitsetStorage<WORDS_PER_SHARD>>,
    const N: usize,
    const SHARDS: usize,
    const WORDS_PER_SHARD: usize,
> Default for InlineKasinoSlotPool<S, N, SHARDS, WORDS_PER_SHARD>
where
    S: Default,
    kasino::strategy::StrategyStakes<S, BitsetStorage<WORDS_PER_SHARD>>: Default,
{
    fn default() -> Self {
        Self::new()
    }
}

/// A handle to an [`InlineKasinoSlotPool`] for thread-local or scoped access.
///
/// This handle provides the [`OwnedSlotPool`], [`RawOwnedSlotPool`], and
/// [`BatchedRawOwnedSlotPool`] trait implementations for interacting with the pool.
///
/// # Type Parameters
/// - `'a`: Lifetime of the handle (tied to the parent pool).
/// - `S`: The scheduling strategy.
/// - `N`: Total number of slots in the pool.
/// - `SHARDS`: Number of shards.
/// - `WORDS_PER_SHARD`: Number of words per shard.
#[allow(private_bounds)]
pub struct InlineKasinoSlotPoolHandle<
    'a,
    S: Strategy<BitsetStorage<WORDS_PER_SHARD>>,
    const N: usize,
    const SHARDS: usize,
    const WORDS_PER_SHARD: usize = WORDS_PER_CACHE_LINE,
> {
    bandit: InlineBanditHandle<
        'a,
        BitsetStorage<WORDS_PER_SHARD>,
        IndexPoolStrategy<S>,
        SHARDS,
        WORDS_PER_SHARD,
    >,
    parent_id: ID,
}

impl<
    'a,
    S: Strategy<BitsetStorage<WORDS_PER_SHARD>>,
    const N: usize,
    const SHARDS: usize,
    const WORDS_PER_SHARD: usize,
> OwnedSlotPoolMeta for InlineKasinoSlotPoolHandle<'a, S, N, SHARDS, WORDS_PER_SHARD>
{
    fn len(&mut self) -> usize {
        self.bandit.len()
    }

    fn capacity(&mut self) -> usize {
        self.bandit.capacity()
    }

    fn is_empty(&mut self) -> bool {
        self.bandit.is_empty()
    }
}

impl<
    'a,
    S: Strategy<BitsetStorage<WORDS_PER_SHARD>>,
    const N: usize,
    const SHARDS: usize,
    const WORDS_PER_SHARD: usize,
> RawOwnedSlotPool for InlineKasinoSlotPoolHandle<'a, S, N, SHARDS, WORDS_PER_SHARD>
{
    fn pull_raw(&mut self) -> Option<usize> {
        self.bandit
            .poll(PullRequest::SingleRequest)
            .ok()
            .map(MaybeBatched::require_single)
    }

    unsafe fn put_raw(&mut self, index: usize) -> bool {
        self.bandit.offer(MaybeBatched::Single(index)).is_ok()
    }
}

impl<
    'a,
    S: Strategy<BitsetStorage<WORDS_PER_SHARD>>,
    const N: usize,
    const SHARDS: usize,
    const WORDS_PER_SHARD: usize,
> BatchedRawOwnedSlotPool for InlineKasinoSlotPoolHandle<'a, S, N, SHARDS, WORDS_PER_SHARD>
{
    fn pull_raw_batch(&mut self) -> Option<RawBatch> {
        self.bandit
            .poll(PullRequest::BatchRequest)
            .ok()
            .map(MaybeBatched::require_batched)
    }

    unsafe fn put_raw_batch(&mut self, batch: RawBatch) -> bool {
        self.bandit.offer(MaybeBatched::Batch(batch)).is_ok()
    }
}

impl<
    'a,
    S: Strategy<BitsetStorage<WORDS_PER_SHARD>>,
    const N: usize,
    const SHARDS: usize,
    const WORDS_PER_SHARD: usize,
> OwnedSlotPool for InlineKasinoSlotPoolHandle<'a, S, N, SHARDS, WORDS_PER_SHARD>
{
    fn id(&mut self) -> ID {
        self.parent_id
    }
}
