use crossbeam_utils::CachePadded;

use crate::{
    core::CoherenceProvider,
    slot_alloc::{
        RawSlotPool,
        SlotHandle,
        SlotPool,
        SlotPoolMeta,
        coherence::AutoCoherenceProvider,
        next_id,
    },
    sync::atomic::Ordering,
};

#[cfg(target_has_atomic = "64")]
pub(crate) type Word = u64;
#[cfg(target_has_atomic = "64")]
pub(crate) type AtomicWord = crate::sync::atomic::AtomicU64;

#[cfg(not(target_has_atomic = "64"))]
pub(crate) type Word = u32;
#[cfg(not(target_has_atomic = "64"))]
pub(crate) type AtomicWord = crate::sync::atomic::AtomicU32;

#[cfg(not(loom))]
#[allow(unused_qualifications)]
pub(crate) const WORD_BYTES: usize = core::mem::size_of::<Word>();
pub(crate) const WORD_BITS: usize = Word::BITS as usize;

#[cfg(not(loom))]
#[allow(unused_qualifications)]
pub(crate) const CACHE_LINE_BYTES: usize = core::mem::align_of::<CachePadded<()>>();
#[cfg(not(loom))]
pub(crate) const WORDS_PER_CACHE_LINE: usize = CACHE_LINE_BYTES / WORD_BYTES;
#[cfg(loom)]
pub(crate) const WORDS_PER_CACHE_LINE: usize = 1;
pub(crate) const BITS_PER_CACHE_LINE: usize = WORDS_PER_CACHE_LINE * WORD_BITS;

pub(crate) struct BitsetStorage {
    words: CachePadded<[AtomicWord; WORDS_PER_CACHE_LINE]>,
}

impl BitsetStorage {
    fn free_count(&self) -> usize {
        self.words
            .iter()
            .map(|w| w.load(Ordering::Acquire).count_ones() as usize)
            .sum()
    }
}

impl Default for BitsetStorage {
    fn default() -> Self {
        Self {
            words: core::array::from_fn(|_| AtomicWord::new(Word::MAX)).into(),
        }
    }
}

impl RawSlotPool for BitsetStorage {
    fn pull_raw(&self) -> Option<usize> {
        for (word_idx, word) in self.words.iter().enumerate() {
            let mut current = word.load(Ordering::Relaxed);

            while current != 0 {
                let bit = current.trailing_zeros();
                let mask = 1 << bit;

                match word.compare_exchange_weak(
                    current,
                    current & !mask,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return Some(word_idx * WORD_BITS + bit as usize),
                    Err(observed) => current = observed,
                }

                #[cfg(any(loom, shuttle))]
                crate::sync::thread::yield_now();
            }
        }

        None
    }

    /// # Safety
    /// index is in bounds and is currently used.
    /// In other words: index is an index retunred by `BitsetStorage::pull_raw` on THIS INSTANCE.
    unsafe fn put_raw(&self, index: usize) -> bool {
        let word_idx = index / WORD_BITS;
        let bit = index % WORD_BITS;
        let mask = 1 << bit;
        // SAFETY:
        // the index is in range of totalbits
        let prev = unsafe { self.words.get_unchecked(word_idx) }.fetch_or(mask, Ordering::Release);
        prev & mask == 0
    }
}

impl SlotPoolMeta for BitsetStorage {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn is_full(&self) -> bool {
        self.len() == BITS_PER_CACHE_LINE
    }

    fn len(&self) -> usize {
        self.free_count()
    }

    fn capacity(&self) -> usize {
        BITS_PER_CACHE_LINE
    }
}

pub(crate) struct MaskedBitsetStorage {
    inner: BitsetStorage,
    usable: u32,
}

impl MaskedBitsetStorage {
    pub(crate) fn new(usable: usize) -> Self {
        debug_assert!(usable <= BITS_PER_CACHE_LINE);
        let inner = BitsetStorage::default();
        for bit in usable..BITS_PER_CACHE_LINE {
            let word_idx = bit / WORD_BITS;
            let b = bit % WORD_BITS;
            inner.words[word_idx].fetch_and(!(1 << b), Ordering::Relaxed);
        }
        Self {
            inner,
            usable: usable as u32,
        }
    }
}

impl RawSlotPool for MaskedBitsetStorage {
    fn pull_raw(&self) -> Option<usize> {
        self.inner.pull_raw()
    }

    /// # Safety
    /// index is in bounds and is currently used.
    /// In other words: index is an index retunred by `MaskedBitsetStorage::pull_raw` on THIS INSTANCE.
    unsafe fn put_raw(&self, index: usize) -> bool {
        if index >= self.usable as usize {
            return false;
        }
        // SAFETY:
        // The index was returned by self.inner.pull_raw()
        unsafe { self.inner.put_raw(index) }
    }
}

impl SlotPoolMeta for MaskedBitsetStorage {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn is_full(&self) -> bool {
        self.len() == self.usable as usize
    }

    fn len(&self) -> usize {
        self.inner.free_count()
    }

    fn capacity(&self) -> usize {
        self.usable as usize
    }
}

pub(crate) trait ShardStorage {
    const SHARD_BITS: usize;
    const SHARD_SHIFT: u32;
    const SHARD_MASK: usize;
}

const _: () = assert!(
    BITS_PER_CACHE_LINE.is_power_of_two(),
    "BITS_PER_CACHE_LINE must be a power of two for bitwise math to work!"
);

impl ShardStorage for BitsetStorage {
    const SHARD_BITS: usize = BITS_PER_CACHE_LINE;
    const SHARD_MASK: usize = BITS_PER_CACHE_LINE - 1;
    const SHARD_SHIFT: u32 = BITS_PER_CACHE_LINE.ilog2();
}

impl ShardStorage for MaskedBitsetStorage {
    const SHARD_BITS: usize = BITS_PER_CACHE_LINE;
    const SHARD_MASK: usize = BITS_PER_CACHE_LINE - 1;
    const SHARD_SHIFT: u32 = BITS_PER_CACHE_LINE.ilog2();
}

pub(crate) struct ConcatStorage<A, B> {
    a: A,
    b: B,
}

impl<A, B> ConcatStorage<A, B> {
    pub(crate) fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<A: Default, B: Default> Default for ConcatStorage<A, B> {
    fn default() -> Self {
        Self {
            a: A::default(),
            b: B::default(),
        }
    }
}

impl<A: RawSlotPool, B: RawSlotPool> RawSlotPool for ConcatStorage<A, B> {
    fn pull_raw(&self) -> Option<usize> {
        self.a
            .pull_raw()
            .or_else(|| self.b.pull_raw().map(|idx| idx + self.a.capacity()))
    }

    /// # Safety
    /// index is in bounds and is currently used.
    /// In other words: index is an index retunred by `ConcatStorage::pull_raw` on THIS INSTANCE.
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

impl<A: SlotPool, B: SlotPool> SlotPool for ConcatStorage<A, B> {
    fn pull(&self) -> Option<SlotHandle> {
        self.a.pull().or_else(|| self.b.pull())
    }

    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        if let Err(handle) = self.a.put(index) {
            return self.b.put(handle);
        }
        Ok(())
    }
}

pub(crate) trait Buffer {
    type Slot;

    fn capacity(&self) -> usize;
    fn inner(&self) -> &[Self::Slot];
}

pub(crate) struct GenericStorage<B, C> {
    buffer: B,
    id: Word,
    coherence_hint: C,
}

impl<B, C: Default> GenericStorage<B, C> {
    pub(crate) fn new(buffer: B) -> Self {
        Self {
            buffer,
            id: next_id(),
            coherence_hint: C::default(),
        }
    }
}

impl<B: Default, C: Default> Default for GenericStorage<B, C> {
    fn default() -> Self {
        Self {
            buffer: B::default(),
            id: next_id(),
            coherence_hint: C::default(),
        }
    }
}

fn unlikely(v: bool) -> bool {
    if v {
        core::hint::cold_path();
    }
    v
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
        self.coherence_hint.advance_hint();

        let mut base_offset = start << B::Slot::SHARD_SHIFT;

        for _ in 0..cap {
            // SAFETY:
            // we ensure that 0 <= start < SHARD SIZE and SHARD_SIZE > 0
            let item = unsafe { inner.get_unchecked(start) };
            if let Some(inner_idx) = item.pull_raw() {
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

impl<B, C> SlotPool for GenericStorage<B, C>
where
    B: Buffer,
    B::Slot: RawSlotPool + ShardStorage,
    C: CoherenceProvider,
{
    fn pull(&self) -> Option<SlotHandle> {
        self.pull_raw().map(|raw| SlotHandle::new(raw, self.id))
    }

    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        if index.id() != self.id {
            return Err(index);
        }
        // SAFETY:
        // we just checked the id
        if unsafe { self.put_raw(index.as_usize()) } {
            Ok(())
        } else {
            Err(index)
        }
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
}

impl<const N: usize, const SHARDS: usize, C: CoherenceProvider> SlotPool
    for InlineSlots<N, SHARDS, C>
{
    fn pull(&self) -> Option<SlotHandle> {
        self.raw.pull()
    }

    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        self.raw.put(index)
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
}

#[cfg(feature = "alloc")]
impl<C: CoherenceProvider> SlotPool for Slots<C> {
    fn pull(&self) -> Option<SlotHandle> {
        self.raw.pull()
    }

    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        self.raw.put(index)
    }
}

/// Computes the numer of shards used to store `n` slots
pub const fn full_shard_count(n: usize) -> usize {
    n / BITS_PER_CACHE_LINE
}

/// Computes how many bits in the last shard should stay unused to sotre exactly `n` slots
pub const fn tail_bits(n: usize) -> usize {
    n % BITS_PER_CACHE_LINE
}
