use crate::{
    Batch,
    SlotHandle,
    SlotPool,
    SlotPoolMeta,
    bitshard::{BITS_PER_CACHE_LINE, BitsetStorage, ShardStorage, WORDS_PER_CACHE_LINE},
    cache_coherence::{AutoCoherenceProvider, CoherenceProvider},
    core_internal::{ID, RawBatch, WORD_BITS, Word},
    slot_alloc::{BatchedRawSlotPool, BatchedSlotPool, RawSlotPool},
    sync::atomic::Ordering,
};

pub(crate) trait Buffer {
    type Slot;

    fn capacity(&self) -> usize;
    fn inner(&self) -> &[Self::Slot];
}

// We do not keep a hint over which shards may currently be non-empty:
// For small capacities, iterating over the shards is almost free, the hint would likely not be much cheaper.
// For large capacities:
// - under high contention such a hint could be expected to help the most, however it would have to be CachePadded itself. This would ~2x memory consumption, which is likely not worth it.
// - under low/no contention a CoherenceProvider can be constructed such that we get a heuristic hint "for free". The defualt RoundRobin implementations supply this already.

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
        debug_assert!(
            buffer.capacity() > 0,
            "The SlotPool should have a capacity greater 0"
        );
        debug_assert_eq!(
            buffer.capacity(),
            capacity.div_ceil(<B::Slot as ShardStorage>::SHARD_BITS),
            "The SlotPool should have the correct number of shards"
        );

        let total_bits = buffer.capacity() * <B::Slot as ShardStorage>::SHARD_BITS;
        let dead_slots = total_bits - capacity;

        if dead_slots > 0 {
            let last_shard = buffer.inner().last().unwrap();
            let words = last_shard.raw_words();

            let tail_bits = capacity % <B::Slot as ShardStorage>::SHARD_BITS;
            let mut valid_words = tail_bits / Word::BITS as usize;
            let rem_bits = tail_bits % WORD_BITS;

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
                // we advance by BITS_PER_CACHE_LINE / WORD_BITS because we want to advance once per consumed word, i.e. every Word::BITS calls
                // we want to advance this often to dodge incoming puts on this shard.
                // in the spin-loop/high throughput benchmark this proved to be the best tradeoff, however under real usecases this may not be true.
                self.coherence_hint
                    .advance_hint_by(BITS_PER_CACHE_LINE / WORD_BITS);
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
}

impl<B, C> BatchedRawSlotPool for GenericStorage<B, C>
where
    B: Buffer,
    B::Slot: ShardStorage + BatchedRawSlotPool,
    C: CoherenceProvider,
{
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
                // we advance by BITS_PER_CACHE_LINE / WORDS_PER_SHARD, because we want to advance once all words in this shard are exhausted, i.e. every WORDS_PER_SHARD calls.
                // this is optimal under workflows where only pull_batch is called, because we only ever pull and put full batches and thus know exactly when to advance.
                // under mixed workflows it is unclear what the best strategy should be.
                self.coherence_hint.advance_hint_by(
                    (BITS_PER_CACHE_LINE * WORD_BITS) / <B::Slot as ShardStorage>::SHARD_BITS,
                );
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

impl<B, C> BatchedSlotPool for GenericStorage<B, C>
where
    B: Buffer,
    B::Slot: ShardStorage + BatchedRawSlotPool,
    C: CoherenceProvider,
{
}

impl<B, C> SlotPool for GenericStorage<B, C>
where
    B: Buffer,
    B::Slot: RawSlotPool + ShardStorage,
    C: CoherenceProvider,
{
    fn id(&self) -> ID {
        self.id
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

/// A statically sized slot storage.
///
/// The storage has a capacity of `N`, distributed over `SHARDS` shards of size _bits in a cacheline_
pub struct InlineSlots<
    const N: usize,
    const SHARDS: usize,
    const WORDS_PER_SHARD: usize = WORDS_PER_CACHE_LINE,
    C = AutoCoherenceProvider,
> {
    raw: GenericStorage<InlineBuffer<BitsetStorage<WORDS_PER_SHARD>, SHARDS>, C>,
}

impl<const N: usize, const SHARDS: usize, const WORDS_PER_SHARD: usize>
    InlineSlots<N, SHARDS, WORDS_PER_SHARD, AutoCoherenceProvider>
{
    /// Constructs a new `InlineSlots` with auto config
    pub fn new() -> Self {
        Self::with_coherence_provider()
    }

    /// Constructs a new `InlineSlots` with the specified coherence provider
    pub fn with_coherence_provider<C: CoherenceProvider + Default>()
    -> InlineSlots<N, SHARDS, WORDS_PER_SHARD, C> {
        InlineSlots {
            raw: GenericStorage::new(InlineBuffer::new(), N),
        }
    }
}

impl<const N: usize, const SHARDS: usize, const WORDS_PER_SHARD: usize> Default
    for InlineSlots<N, SHARDS, WORDS_PER_SHARD, AutoCoherenceProvider>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, const SHARDS: usize, const WORDS_PER_SHARD: usize, C> SlotPoolMeta
    for InlineSlots<N, SHARDS, WORDS_PER_SHARD, C>
{
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

impl<const N: usize, const SHARDS: usize, const WORDS_PER_SHARD: usize, C: CoherenceProvider>
    RawSlotPool for InlineSlots<N, SHARDS, WORDS_PER_SHARD, C>
{
    fn pull_raw(&self) -> Option<usize> {
        self.raw.pull_raw()
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        // SAFETY:
        // index was returned by self.pull_raw
        unsafe { self.raw.put_raw(index) }
    }
}

impl<const N: usize, const SHARDS: usize, const WORDS_PER_SHARD: usize, C: CoherenceProvider>
    BatchedRawSlotPool for InlineSlots<N, SHARDS, WORDS_PER_SHARD, C>
{
    fn pull_raw_batch(&self) -> Option<RawBatch> {
        self.raw.pull_raw_batch()
    }

    unsafe fn put_raw_batch(&self, batch: RawBatch) -> bool {
        // SAFETY:
        // The caller promises that this batch is valid
        unsafe { self.raw.put_raw_batch(batch) }
    }
}

impl<const N: usize, const SHARDS: usize, const WORDS_PER_SHARD: usize, C: CoherenceProvider>
    SlotPool for InlineSlots<N, SHARDS, WORDS_PER_SHARD, C>
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
}

impl<const N: usize, const SHARDS: usize, const WORDS_PER_SHARD: usize, C: CoherenceProvider>
    BatchedSlotPool for InlineSlots<N, SHARDS, WORDS_PER_SHARD, C>
{
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

/// A dynamically sized, allocated index storage.
#[cfg(feature = "alloc")]
pub struct Slots<C = AutoCoherenceProvider> {
    raw: GenericStorage<HeapBuf<BitsetStorage>, C>,
}

#[cfg(feature = "alloc")]
impl Slots<AutoCoherenceProvider> {
    /// Constructs a new `Slots` instance with capacity `size`
    pub fn new(size: usize) -> Self {
        Self::with_coherence_provider(size)
    }

    /// Constructs a new `Slots` instance with specified coherence provider.
    pub fn with_coherence_provider<C: CoherenceProvider + Default>(size: usize) -> Slots<C> {
        Slots {
            raw: GenericStorage::new(HeapBuf::new(size.div_ceil(BITS_PER_CACHE_LINE)), size),
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
}

#[cfg(feature = "alloc")]
impl<C: CoherenceProvider> BatchedRawSlotPool for Slots<C> {
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
}

#[cfg(feature = "alloc")]
impl<C: CoherenceProvider> BatchedSlotPool for Slots<C> {
    fn pull_batch(&self) -> Option<Batch> {
        self.raw.pull_batch()
    }

    fn put_batch(&self, batch: Batch) -> Result<(), Batch> {
        self.raw.put_batch(batch)
    }
}

#[cfg(feature = "word-slots")]
#[doc(hidden)]
pub mod batched {
    use super::*;

    /// A wrapper around SlotPools, which retinerepts Batches as Slots,
    /// allowing some performance optimizations in some cases
    #[allow(unnameable_types)]
    #[doc(hidden)]
    #[repr(transparent)]
    pub struct WordPool<P> {
        inner: P,
    }

    impl<P> WordPool<P> {
        /// Creates a new WordPool instance over slot pool P
        pub(crate) fn new_in(inner: P) -> Self {
            Self { inner }
        }
    }

    impl<P: SlotPoolMeta> SlotPoolMeta for WordPool<P> {
        fn len(&self) -> usize {
            self.inner.len() / WORD_BITS
        }

        fn capacity(&self) -> usize {
            self.inner.capacity() / WORD_BITS
        }

        fn is_empty(&self) -> bool {
            self.inner.len() < WORD_BITS
        }

        fn is_full(&self) -> bool {
            (self.inner.capacity() - self.inner.len()) < WORD_BITS
        }
    }

    impl<P: BatchedRawSlotPool> RawSlotPool for WordPool<P> {
        fn pull_raw(&self) -> Option<usize> {
            let inner_batch = self.inner.pull_raw_batch()?;
            let word_idx = inner_batch.starting_idx / WORD_BITS;
            Some(word_idx)
        }

        unsafe fn put_raw(&self, index: usize) -> bool {
            let bit_idx = index * WORD_BITS;

            let full_word_batch = RawBatch {
                starting_idx: bit_idx,
                mask: Word::MAX,
            };

            // SAFETY: Caller guarantees batch validity
            unsafe { self.inner.put_raw_batch(full_word_batch) }
        }
    }

    impl<P: BatchedSlotPool> SlotPool for WordPool<P> {
        fn id(&self) -> ID {
            self.inner.id()
        }
    }

    impl<const BIT_CAPACITY: usize, const SHARDS: usize, const WORDS_PER_SHARD: usize>
        WordPool<InlineSlots<BIT_CAPACITY, SHARDS, WORDS_PER_SHARD, AutoCoherenceProvider>>
    {
        /// Constructs a new Inlined Word Pool
        pub fn new() -> Self {
            Self::with_coherence_provider()
        }

        /// Constructs a new `Slots` instance with specified coherence provider.
        pub fn with_coherence_provider<C: CoherenceProvider + Default>()
        -> WordPool<InlineSlots<BIT_CAPACITY, SHARDS, WORDS_PER_SHARD, C>> {
            WordPool::new_in(InlineSlots::with_coherence_provider())
        }
    }

    impl<const BIT_CAPACITY: usize, const SHARDS: usize, const WORDS_PER_SHARD: usize> Default
        for WordPool<InlineSlots<BIT_CAPACITY, SHARDS, WORDS_PER_SHARD, AutoCoherenceProvider>>
    {
        fn default() -> Self {
            Self::new()
        }
    }

    /// A word-granularity heap-allocated slot storage.
    /// Slots are stored as words.
    #[cfg(feature = "alloc")]
    pub type WordSlots<C = AutoCoherenceProvider> = WordPool<Slots<C>>;

    #[cfg(feature = "alloc")]
    impl WordSlots<AutoCoherenceProvider> {
        /// Constructs a new `WordSlots` instance
        pub fn new(size: usize) -> Self {
            Self::with_coherence_provider(size)
        }

        /// Constructs a new `WordSlots` instance with specified coherence provider.
        pub fn with_coherence_provider<C: CoherenceProvider + Default>(
            size: usize,
        ) -> WordSlots<C> {
            WordPool::new_in(Slots::with_coherence_provider(size * WORD_BITS))
        }
    }
}

#[cfg(all(test, not(miri), not(loom), not(shuttle)))]
mod tests {
    #[cfg(all(feature = "word-slots", feature = "alloc"))]
    use super::batched::WordSlots;
    #[cfg(feature = "alloc")]
    use crate::Slots;
    use crate::define_inline_slots;
    #[cfg(feature = "word-slots")]
    use crate::define_inline_wordslots;

    fn assert_send_sync<S>(_: S)
    where
        S: Send + Sync,
    {
    }

    #[test]
    fn send_sync_pools() {
        define_inline_slots!(SlotPool, 1);
        assert_send_sync(SlotPool::new());
        #[cfg(feature = "alloc")]
        assert_send_sync(Slots::new(1));
        #[cfg(feature = "word-slots")]
        define_inline_wordslots!(WordSlotPool, 1);
        #[cfg(feature = "word-slots")]
        assert_send_sync(WordSlotPool::new());
        #[cfg(all(feature = "word-slots", feature = "alloc"))]
        assert_send_sync(WordSlots::new(1));
    }
}
