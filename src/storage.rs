use crate::{
    Batch,
    SlotHandle,
    SlotPool,
    SlotPoolMeta,
    bitshard::{BITS_PER_CACHE_LINE, BitsetStorage, MaskedBitsetStorage, ShardStorage},
    cache_coherence::{AutoCoherenceProvider, CoherenceProvider},
    core::{ID, RawBatch, RawSlotPool, Word, tail_bits},
    core_internal::unlikely,
};

pub(crate) struct ConcatStorage<A, B> {
    a: A,
    b: B,
    id: ID,
}

impl<A, B> ConcatStorage<A, B> {
    pub(crate) fn new(a: A, b: B) -> Self {
        Self {
            a,
            b,
            id: ID::next(),
        }
    }
}

impl<A: Default, B: Default> Default for ConcatStorage<A, B> {
    fn default() -> Self {
        Self {
            a: A::default(),
            b: B::default(),
            id: ID::next(),
        }
    }
}

impl<A: RawSlotPool, B: RawSlotPool> RawSlotPool for ConcatStorage<A, B> {
    fn pull_raw(&self) -> Option<usize> {
        self.a
            .pull_raw()
            .or_else(|| self.b.pull_raw().map(|idx| idx + self.a.capacity()))
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        let a_cap = self.a.capacity();
        if index < a_cap {
            // SAFETY:
            // The index was returned by self.inner.pull_raw()
            // We just checked that it is within bounds of the allocation
            unsafe { self.a.put_raw(index) }
        } else {
            // SAFETY:
            // The index was returned by self.inner.pull_raw()
            // Thus it is within bounds of the allocation
            unsafe { self.b.put_raw(index - a_cap) }
        }
    }

    fn pull_raw_batch(&self) -> Option<RawBatch> {
        self.a.pull_raw_batch().or_else(|| {
            self.b.pull_raw_batch().map(|mut batch| {
                batch.starting_idx += self.a.capacity();
                batch
            })
        })
    }

    unsafe fn put_raw_batch(&self, batch: RawBatch) -> bool {
        let a_cap = self.a.capacity();

        if batch.starting_idx < a_cap {
            // SAFETY:
            // The index was returned by self.inner.pull_raw_batch()
            // We just checked that it is within bounds of the allocation
            unsafe { self.a.put_raw_batch(batch) }
        } else {
            // SAFETY:
            // The index was returned by self.inner.pull_raw_batch()
            // Thus it is within bounds of the allocation
            unsafe {
                self.b.put_raw_batch(RawBatch {
                    starting_idx: batch.starting_idx - a_cap,
                    mask: batch.mask,
                })
            }
        }
    }
}

impl<A: SlotPoolMeta, B: SlotPoolMeta> SlotPoolMeta for ConcatStorage<A, B> {
    fn is_empty(&self) -> bool {
        self.a.is_empty() && self.b.is_empty()
    }

    fn is_full(&self) -> bool {
        self.a.is_full() && self.b.is_full()
    }

    fn len(&self) -> usize {
        self.a.len() + self.b.len()
    }

    fn capacity(&self) -> usize {
        self.a.capacity() + self.b.capacity()
    }
}

impl<A: RawSlotPool, B: RawSlotPool> SlotPool for ConcatStorage<A, B> {
    fn id(&self) -> ID {
        self.id.clone()
    }

    fn pull(&self) -> Option<SlotHandle> {
        self.pull_raw()
            .map(|slot| SlotHandle::new(slot, self.id.clone()))
    }

    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        if *index.id() != self.id {
            return Err(index);
        }
        // SAFETY:
        // we just checked that this batch was created by us.
        // Capacity checks will be performed by put_raw
        if unsafe { self.put_raw(index.as_usize()) } {
            Ok(())
        } else {
            Err(index)
        }
    }

    fn pull_batch(&self) -> Option<Batch> {
        self.pull_raw_batch()
            .map(|batch| Batch::new(self.id.clone(), batch))
    }

    fn put_batch(&self, batch: Batch) -> Result<(), Batch> {
        if *batch.id() != self.id {
            return Err(batch);
        }
        // SAFETY:
        // we just checked that this batch was created by us.
        // Capacity checks will be performed by put_raw_batch
        if unsafe { self.put_raw_batch(*batch.raw()) } {
            Ok(())
        } else {
            Err(batch)
        }
    }
}

pub(crate) trait Buffer {
    type Slot;

    fn capacity(&self) -> usize;
    fn inner(&self) -> &[Self::Slot];
}

pub(crate) struct GenericStorage<B, C> {
    buffer: B,
    coherence_hint: C,
}

impl<B, C: Default> GenericStorage<B, C> {
    pub(crate) fn new(buffer: B) -> Self {
        Self {
            buffer,
            coherence_hint: C::default(),
        }
    }
}

impl<B: Default, C: Default> Default for GenericStorage<B, C> {
    fn default() -> Self {
        Self {
            buffer: B::default(),
            coherence_hint: C::default(),
        }
    }
}

impl<B, C> RawSlotPool for GenericStorage<B, C>
where
    B: Buffer,
    B::Slot: ShardStorage + RawSlotPool,
    C: CoherenceProvider,
{
    fn pull_raw(&self) -> Option<usize> {
        let inner = self.buffer.inner();
        let cap = self.buffer.capacity();
        // TODO: move to constructor
        if unlikely(cap == 0) {
            return None;
        }

        let mut start = self.coherence_hint.current_hint() % cap;

        let mut base_offset = start << B::Slot::SHARD_SHIFT;

        for _ in 0..cap {
            // SAFETY:
            // we ensure that 0 <= start < SHARD SIZE and SHARD_SIZE > 0
            let item = unsafe { inner.get_unchecked(start) };
            if let Some(inner_idx) = item.pull_raw() {
                self.coherence_hint
                    .advance_hint_by(BITS_PER_CACHE_LINE / Word::BITS as usize);
                return Some(base_offset + inner_idx);
            }

            start += 1;
            base_offset += B::Slot::SHARD_BITS;
            if start == cap {
                start = 0;
                base_offset = 0;
            }
        }

        None
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        let inner = self.buffer.inner();
        // TODO: move to constructor
        if unlikely(self.buffer.capacity() == 0) {
            return false;
        }

        let row = index >> B::Slot::SHARD_SHIFT;
        let col = index & B::Slot::SHARD_MASK;

        // SAFETY:
        // index is a valid index as returned by `pull_raw`
        let slot = unsafe { inner.get_unchecked(row) };
        // SAFETY:
        // we ensure that 0 <= col < SHARD SIZE and SHARD_SIZE > 0,
        // given that index is valid
        unsafe { slot.put_raw(col) }
    }

    fn pull_raw_batch(&self) -> Option<RawBatch> {
        let inner = self.buffer.inner();
        let cap = self.buffer.capacity();
        // TODO: move to constructor
        if unlikely(cap == 0) {
            return None;
        }

        let mut start = self.coherence_hint.current_hint() % cap;

        let mut base_offset = start << B::Slot::SHARD_SHIFT;

        for _ in 0..cap {
            // SAFETY:
            // we ensure that 0 <= start < SHARD SIZE and SHARD_SIZE > 0
            let item = unsafe { inner.get_unchecked(start) };
            if let Some(mut inner_batch) = item.pull_raw_batch() {
                inner_batch.starting_idx += base_offset;
                self.coherence_hint.advance_hint_by(Word::BITS as usize);
                return Some(inner_batch);
            }

            start += 1;
            base_offset += B::Slot::SHARD_BITS;
            if start == cap {
                start = 0;
                base_offset = 0;
            }
        }

        None
    }

    unsafe fn put_raw_batch(&self, batch: RawBatch) -> bool {
        let inner = self.buffer.inner();
        // TODO: move to constructor
        if unlikely(self.buffer.capacity() == 0) {
            return false;
        }

        let row = batch.starting_idx >> B::Slot::SHARD_SHIFT;
        let col = batch.starting_idx & B::Slot::SHARD_MASK;

        // SAFETY:
        // index is a valid index as returned by `pull_raw`
        let slot = unsafe { inner.get_unchecked(row) };
        // SAFETY:
        // we ensure that 0 <= col < SHARD SIZE and SHARD_SIZE > 0,
        // given that index is valid
        unsafe {
            slot.put_raw_batch(RawBatch {
                starting_idx: col,
                mask: batch.mask,
            })
        }
    }

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

impl<B, C> SlotPoolMeta for GenericStorage<B, C>
where
    B: Buffer,
    B::Slot: SlotPoolMeta + ShardStorage,
{
    fn is_empty(&self) -> bool {
        self.buffer.inner().iter().all(|slot| slot.is_empty())
    }

    fn is_full(&self) -> bool {
        self.buffer.inner().iter().all(|slot| slot.is_full())
    }

    fn len(&self) -> usize {
        self.buffer.inner().iter().map(|slot| slot.len()).sum()
    }

    fn capacity(&self) -> usize {
        self.buffer.inner().iter().map(|slot| slot.capacity()).sum()
    }
}

pub(crate) struct InlineBuffer<T, const N: usize> {
    buf: [T; N],
}

impl<T: Default, const N: usize> InlineBuffer<T, N> {
    pub(crate) fn new() -> Self {
        Self {
            buf: core::array::from_fn(|_| T::default()),
        }
    }
}

impl<T> InlineBuffer<T, 1> {
    pub(crate) fn with_storage(storage: T) -> Self {
        Self { buf: [storage] }
    }
}

impl<T, const N: usize> Buffer for InlineBuffer<T, N> {
    type Slot = T;

    fn capacity(&self) -> usize {
        N
    }

    fn inner(&self) -> &[Self::Slot] {
        self.buf.as_ref()
    }
}

/// A statically sized slot storage stored on the stack.
///
/// The storage has a capacity of `N`, distributed over `SHARDS` shards of size _bits in a cacheline_
pub struct InlineSlots<const N: usize, const SHARDS: usize, C = AutoCoherenceProvider> {
    raw: ConcatStorage<
        GenericStorage<InlineBuffer<BitsetStorage, SHARDS>, C>,
        GenericStorage<InlineBuffer<MaskedBitsetStorage, 1>, C>,
    >,
}

impl<const N: usize, const SHARDS: usize> InlineSlots<N, SHARDS, AutoCoherenceProvider> {
    /// Constructs a new `InlineSlots` with auto config
    pub fn new() -> Self {
        Self::with_coherence_provider()
    }

    /// Constructs a new `InlineSlots` with the specified coherence provider
    pub fn with_coherence_provider<C: CoherenceProvider + Default>() -> InlineSlots<N, SHARDS, C> {
        InlineSlots {
            raw: ConcatStorage::new(
                GenericStorage::new(InlineBuffer::new()),
                GenericStorage::new(InlineBuffer::with_storage(MaskedBitsetStorage::new(
                    tail_bits(N),
                ))),
            ),
        }
    }
}

impl<const N: usize, const SHARDS: usize> Default
    for InlineSlots<N, SHARDS, AutoCoherenceProvider>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, const SHARDS: usize, C> SlotPoolMeta for InlineSlots<N, SHARDS, C> {
    fn len(&self) -> usize {
        self.raw.len()
    }

    fn capacity(&self) -> usize {
        self.raw.capacity()
    }

    fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    fn is_full(&self) -> bool {
        self.raw.is_full()
    }
}

impl<const N: usize, const SHARDS: usize, C: CoherenceProvider> RawSlotPool
    for InlineSlots<N, SHARDS, C>
{
    fn pull_raw(&self) -> Option<usize> {
        self.raw.pull_raw()
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        // SAFETY:
        // index was returned by self.pull_raw
        unsafe { self.raw.put_raw(index) }
    }

    fn pull_raw_batch(&self) -> Option<RawBatch> {
        self.raw.pull_raw_batch()
    }

    unsafe fn put_raw_batch(&self, batch: RawBatch) -> bool {
        // SAFETY:
        // The caller promises that this batch is valid
        unsafe { self.raw.put_raw_batch(batch) }
    }
}

impl<const N: usize, const SHARDS: usize, C: CoherenceProvider> SlotPool
    for InlineSlots<N, SHARDS, C>
{
    fn id(&self) -> ID {
        self.raw.id()
    }

    fn pull(&self) -> Option<SlotHandle> {
        self.raw.pull()
    }

    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        self.raw.put(index)
    }

    fn pull_batch(&self) -> Option<Batch> {
        self.raw.pull_batch()
    }

    fn put_batch(&self, batch: Batch) -> Result<(), Batch> {
        self.raw.put_batch(batch)
    }
}

#[cfg(feature = "alloc")]
pub(crate) struct HeapBuf<T> {
    #[allow(unused_qualifications)]
    raw: alloc::boxed::Box<[T]>,
}

#[cfg(feature = "alloc")]
impl<T: Default> HeapBuf<T> {
    pub(crate) fn new(size: usize) -> Self {
        Self {
            raw: (0..size).map(|_| T::default()).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl<T> Buffer for HeapBuf<T> {
    type Slot = T;

    fn capacity(&self) -> usize {
        self.raw.len()
    }

    fn inner(&self) -> &[Self::Slot] {
        self.raw.as_ref()
    }
}

/// A statically sized index storage stored on the heap.
#[cfg(feature = "alloc")]
pub struct Slots<C> {
    raw: ConcatStorage<
        GenericStorage<HeapBuf<BitsetStorage>, C>,
        GenericStorage<InlineBuffer<MaskedBitsetStorage, 1>, C>,
    >,
}

#[cfg(feature = "alloc")]
impl Slots<AutoCoherenceProvider> {
    /// Constructs a new `Slots` instance with capacity `size`
    pub fn new(size: usize) -> Self {
        Self::with_coherence_provider(size)
    }

    /// COnstructs a new `Slots` instance with specified coherence provider.
    pub fn with_coherence_provider<C: CoherenceProvider + Default>(size: usize) -> Slots<C> {
        use crate::core::full_shard_count;

        Slots {
            raw: ConcatStorage::new(
                GenericStorage::new(HeapBuf::new(full_shard_count(size))),
                GenericStorage::new(InlineBuffer::with_storage(MaskedBitsetStorage::new(
                    tail_bits(size),
                ))),
            ),
        }
    }
}

#[cfg(feature = "alloc")]
impl<C> SlotPoolMeta for Slots<C> {
    fn len(&self) -> usize {
        self.raw.len()
    }

    fn capacity(&self) -> usize {
        self.raw.capacity()
    }

    fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    fn is_full(&self) -> bool {
        self.raw.is_full()
    }
}

#[cfg(feature = "alloc")]
impl<C: CoherenceProvider> RawSlotPool for Slots<C> {
    fn pull_raw(&self) -> Option<usize> {
        self.raw.pull_raw()
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        // SAFETY:
        // index was returned by self.pull_raw
        unsafe { self.raw.put_raw(index) }
    }

    fn pull_raw_batch(&self) -> Option<RawBatch> {
        self.raw.pull_raw_batch()
    }

    unsafe fn put_raw_batch(&self, batch: RawBatch) -> bool {
        // SAFETY:
        // the caller promises that this batch is valid
        unsafe { self.raw.put_raw_batch(batch) }
    }
}

#[cfg(feature = "alloc")]
impl<C: CoherenceProvider> SlotPool for Slots<C> {
    fn id(&self) -> ID {
        self.raw.id()
    }

    fn pull(&self) -> Option<SlotHandle> {
        self.raw.pull()
    }

    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        self.raw.put(index)
    }

    fn pull_batch(&self) -> Option<Batch> {
        self.raw.pull_batch()
    }

    fn put_batch(&self, batch: Batch) -> Result<(), Batch> {
        self.raw.put_batch(batch)
    }
}
