use crate::{
    Batch,
    SlotHandle,
    SlotPool,
    SlotPoolMeta,
    bitshard::{BITS_PER_CACHE_LINE, BitsetStorage, ShardStorage},
    cache_coherence::{AutoCoherenceProvider, CoherenceProvider},
    core::{ID, RawBatch, RawSlotPool, Word},
    sync::atomic::Ordering,
};

pub(crate) trait Buffer {
    type Slot;

    fn capacity(&self) -> usize;
    fn inner(&self) -> &[Self::Slot];
}

pub(crate) struct GenericStorage<B, C> {
    buffer: B,
    coherence_hint: C,
    id: ID,
    capacity: usize,
}

impl<B, C> GenericStorage<B, C>
where
    C: Default,
    B: Buffer,
    B::Slot: ShardStorage,
{
    pub(crate) fn new(buffer: B, capacity: usize) -> Self {
        assert!(
            buffer.capacity() > 0,
            "The SlotPool should have a capacity greater 0"
        );
        let total_bits = buffer.capacity() * <B::Slot as ShardStorage>::SHARD_BITS;
        let dead_slots = total_bits - capacity;

        if dead_slots > 0 {
            let last_shard = buffer.inner().last().unwrap();
            let words = last_shard.raw_words();

            let tail_bits = capacity % <B::Slot as ShardStorage>::SHARD_BITS;
            let mut valid_words = tail_bits / Word::BITS as usize;
            let rem_bits = tail_bits % Word::BITS as usize;

            if rem_bits > 0 {
                let mask = (1 << rem_bits) - 1;
                words[valid_words].fetch_and(mask, Ordering::Relaxed);
                valid_words += 1;
            }
            for b in &words[valid_words..] {
                b.fetch_and(0, Ordering::Relaxed);
            }
        }

        Self {
            buffer,
            coherence_hint: C::default(),
            id: ID::next(),
            capacity,
        }
    }
}

impl<B: Default + Buffer, C: Default> Default for GenericStorage<B, C> {
    fn default() -> Self {
        let buffer = B::default();
        let capacity = buffer.capacity();
        Self {
            buffer,
            coherence_hint: C::default(),
            id: ID::next(),
            capacity,
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
}

impl<B, C> SlotPool for GenericStorage<B, C>
where
    B: Buffer,
    B::Slot: RawSlotPool + ShardStorage,
    C: CoherenceProvider,
{
    fn id(&self) -> ID {
        self.id.clone()
    }
}

impl<B, C> SlotPoolMeta for GenericStorage<B, C>
where
    B: Buffer,
    B::Slot: SlotPoolMeta + ShardStorage,
{
    fn len(&self) -> usize {
        self.buffer
            .inner()
            .iter()
            .map(|slot| slot.len())
            .sum::<usize>()
    }

    fn capacity(&self) -> usize {
        self.capacity
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

#[allow(unused)]
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
    raw: GenericStorage<InlineBuffer<BitsetStorage, SHARDS>, C>,
}

impl<const N: usize, const SHARDS: usize> InlineSlots<N, SHARDS, AutoCoherenceProvider> {
    /// Constructs a new `InlineSlots` with auto config
    pub fn new() -> Self {
        Self::with_coherence_provider()
    }

    /// Constructs a new `InlineSlots` with the specified coherence provider
    pub fn with_coherence_provider<C: CoherenceProvider + Default>() -> InlineSlots<N, SHARDS, C> {
        assert!(
            SHARDS * BITS_PER_CACHE_LINE >= N,
            "InlineSlots: SHARDS ({SHARDS}) is too small to hold capacity N ({N}). Required shards: {}",
            crate::bitshard::full_shard_count(N)
        );
        InlineSlots {
            raw: GenericStorage::new(InlineBuffer::new(), N),
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
    raw: GenericStorage<HeapBuf<BitsetStorage>, C>,
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
            raw: GenericStorage::new(HeapBuf::new(full_shard_count(size)), size),
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
