use crossbeam_utils::CachePadded;

use crate::{
    slot_alloc::{RawStorage, SlotHandle, StorageData, StorageExt, next_id},
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

// TODO 32 bit atomcis under cfg

const WORD_BITS: usize = u64::BITS as usize;

pub struct BitsetStorage<const WORDS: usize> {
    words: CachePadded<[AtomicU64; WORDS]>,
}

impl<const WORDS: usize> BitsetStorage<WORDS> {
    pub const BITS: usize = WORDS * WORD_BITS;

    fn free_count(&self) -> usize {
        self.words
            .iter()
            .map(|w| w.load(Ordering::Acquire).count_ones() as usize)
            .sum()
    }
}

impl<const WORDS: usize> Default for BitsetStorage<WORDS> {
    fn default() -> Self {
        Self {
            words: core::array::from_fn(|_| AtomicU64::new(u64::MAX)).into(),
        }
    }
}

impl<const WORDS: usize> RawStorage for BitsetStorage<WORDS> {
    fn pull_raw(&self) -> Option<usize> {
        for (word_idx, word) in self.words.iter().enumerate() {
            let mut current = word.load(Ordering::Relaxed);

            while current != 0 {
                let bit = current.trailing_zeros();
                let mask = 1u64 << bit;

                match word.compare_exchange_weak(
                    current,
                    current & !mask,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return Some(word_idx * WORD_BITS + bit as usize),
                    Err(observed) => current = observed,
                }
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
        let mask = 1u64 << bit;
        // SAFETY:
        // the index is in range of totalbits
        let prev = unsafe { self.words.get_unchecked(word_idx) }.fetch_or(mask, Ordering::AcqRel);
        prev & mask == 0
    }
}

impl<const WORDS: usize> StorageData for BitsetStorage<WORDS> {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn is_full(&self) -> bool {
        self.len() == Self::BITS
    }

    fn len(&self) -> usize {
        Self::BITS - self.free_count()
    }

    fn capacity(&self) -> usize {
        Self::BITS
    }
}

pub struct MaskedBitsetStorage<const WORDS: usize> {
    inner: BitsetStorage<WORDS>,
    usable: u32,
}

impl<const WORDS: usize> MaskedBitsetStorage<WORDS> {
    pub fn new(usable: usize) -> Self {
        debug_assert!(usable <= BitsetStorage::<WORDS>::BITS);
        let inner = BitsetStorage::default();
        for bit in usable..BitsetStorage::<WORDS>::BITS {
            let word_idx = bit / WORD_BITS;
            let b = bit % WORD_BITS;
            inner.words[word_idx].fetch_and(!(1u64 << b), Ordering::Relaxed);
        }
        Self {
            inner,
            usable: usable as u32,
        }
    }
}

impl<const WORDS: usize> RawStorage for MaskedBitsetStorage<WORDS> {
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

impl<const WORDS: usize> StorageData for MaskedBitsetStorage<WORDS> {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn is_full(&self) -> bool {
        self.len() == self.usable as usize
    }

    fn len(&self) -> usize {
        self.usable as usize - self.inner.free_count()
    }

    fn capacity(&self) -> usize {
        self.usable as usize
    }
}

pub struct ConcatStorage<A, B> {
    a: A,
    b: B,
}

impl<A, B> ConcatStorage<A, B> {
    pub fn new(a: A, b: B) -> Self {
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

impl<A: RawStorage, B: RawStorage> RawStorage for ConcatStorage<A, B> {
    fn pull_raw(&self) -> Option<usize> {
        if let Some(idx) = self.a.pull_raw() {
            return Some(idx);
        }
        self.b.pull_raw().map(|idx| idx + self.a.capacity())
    }

    /// # Safety
    /// index is in bounds and is currently used.
    /// In other words: index is an index retunred by `ConcatStorage::pull_raw` on THIS INSTANCE.
    unsafe fn put_raw(&self, index: usize) -> bool {
        let a_cap = self.a.capacity();
        if index < a_cap {
            unsafe { self.a.put_raw(index) }
        } else {
            unsafe { self.b.put_raw(index - a_cap) }
        }
    }
}

impl<A: StorageData, B: StorageData> StorageData for ConcatStorage<A, B> {
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

impl<A: StorageExt, B: StorageExt> StorageExt for ConcatStorage<A, B> {
    fn pull(&self) -> Option<SlotHandle> {
        if let Some(r) = self.a.pull() {
            Some(r)
        } else {
            self.b.pull()
        }
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

pub struct GenericStorage<B> {
    buffer: B,
    cursor: CachePadded<AtomicUsize>,
    id: u64,
}

impl<B> GenericStorage<B> {
    pub fn new(buffer: B) -> Self {
        Self {
            buffer,
            cursor: AtomicUsize::new(0).into(),
            id: next_id(),
        }
    }
}

impl<B: Default> Default for GenericStorage<B> {
    fn default() -> Self {
        Self {
            buffer: B::default(),
            cursor: AtomicUsize::new(0).into(),
            id: next_id(),
        }
    }
}

impl<B> RawStorage for GenericStorage<B>
where
    B: Buffer,
    B::Slot: RawStorage,
{
    fn pull_raw(&self) -> Option<usize> {
        if self.buffer.capacity() == 0 {
            return None;
        }
        let start = self.cursor.fetch_add(1, Ordering::Relaxed) % self.buffer.capacity();
        for offset in 0..self.buffer.capacity() {
            let idx = (start + offset) % self.buffer.capacity();
            let item = &self.buffer.inner()[idx];
            if let Some(inner_idx) = item.pull_raw() {
                return Some(inner_idx + idx * item.capacity());
            }
        }
        None
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        if self.buffer.capacity() == 0 {
            return false;
        }
        let inner_capacity = self.buffer.inner()[0].capacity();
        if inner_capacity == 0 {
            return false;
        }
        let row = index / inner_capacity;
        let col = index % inner_capacity;
        self.buffer
            .inner()
            .get(row)
            .map(|slot| unsafe { slot.put_raw(col) })
            .unwrap_or(false)
    }
}

impl<B> StorageData for GenericStorage<B>
where
    B: Buffer,
    B::Slot: StorageData,
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

impl<B> StorageExt for GenericStorage<B>
where
    B: Buffer,
    B::Slot: RawStorage,
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

pub struct InlineBuffer<T, const N: usize> {
    buf: [T; N],
}

impl<T: Default, const N: usize> InlineBuffer<T, N> {
    pub fn new() -> Self {
        Self {
            buf: core::array::from_fn(|_| T::default()),
        }
    }
}

impl<T> InlineBuffer<T, 1> {
    pub fn with_storage(storage: T) -> Self {
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

pub struct InlineStorage<const N: usize, const SHARDS: usize, const WORDS: usize> {
    raw: ConcatStorage<
        GenericStorage<InlineBuffer<BitsetStorage<WORDS>, SHARDS>>,
        GenericStorage<InlineBuffer<MaskedBitsetStorage<WORDS>, 1>>,
    >,
}

impl<const N: usize, const SHARDS: usize, const WORDS: usize> InlineStorage<N, SHARDS, WORDS> {
    pub fn new() -> Self {
        Self {
            raw: ConcatStorage::new(
                GenericStorage::new(InlineBuffer::new()),
                GenericStorage::new(InlineBuffer::with_storage(MaskedBitsetStorage::new(
                    tail_bits(N, WORDS * WORD_BITS),
                ))),
            ),
        }
    }
}

impl<const N: usize, const SHARDS: usize, const WORDS: usize> StorageData
    for InlineStorage<N, SHARDS, WORDS>
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

impl<const N: usize, const SHARDS: usize, const WORDS: usize> RawStorage
    for InlineStorage<N, SHARDS, WORDS>
{
    fn pull_raw(&self) -> Option<usize> {
        self.raw.pull_raw()
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        unsafe { self.raw.put_raw(index) }
    }
}

impl<const N: usize, const SHARDS: usize, const WORDS: usize> StorageExt
    for InlineStorage<N, SHARDS, WORDS>
{
    fn pull(&self) -> Option<SlotHandle> {
        self.raw.pull()
    }

    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        self.raw.put(index)
    }
}

#[cfg(feature = "alloc")]
pub struct HeapBuf<T> {
    raw: Box<[T]>,
}

#[cfg(feature = "alloc")]
impl<T: Default> HeapBuf<T> {
    pub fn new(size: usize) -> Self {
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

#[cfg(feature = "alloc")]
pub struct HeapStorage<const WORDS: usize> {
    raw: ConcatStorage<
        GenericStorage<HeapBuf<BitsetStorage<WORDS>>>,
        GenericStorage<InlineBuffer<MaskedBitsetStorage<WORDS>, 1>>,
    >,
}

#[cfg(feature = "alloc")]
impl<const WORDS: usize> HeapStorage<WORDS> {
    pub fn new(size: usize) -> Self {
        Self {
            raw: ConcatStorage::new(
                GenericStorage::new(HeapBuf::new(full_shard_count(size, WORD_BITS * WORDS))),
                GenericStorage::new(InlineBuffer::with_storage(MaskedBitsetStorage::new(
                    tail_bits(size, WORD_BITS * WORDS),
                ))),
            ),
        }
    }
}

#[cfg(feature = "alloc")]
impl<const WORDS: usize> StorageData for HeapStorage<WORDS> {
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
impl<const WORDS: usize> RawStorage for HeapStorage<WORDS> {
    fn pull_raw(&self) -> Option<usize> {
        self.raw.pull_raw()
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        unsafe { self.raw.put_raw(index) }
    }
}

#[cfg(feature = "alloc")]
impl<const WORDS: usize> StorageExt for HeapStorage<WORDS> {
    fn pull(&self) -> Option<SlotHandle> {
        self.raw.pull()
    }

    fn put(&self, index: SlotHandle) -> Result<(), SlotHandle> {
        self.raw.put(index)
    }
}

pub const fn full_shard_count(n: usize, shard_bits: usize) -> usize {
    n / shard_bits
}

pub const fn tail_bits(n: usize, shard_bits: usize) -> usize {
    n % shard_bits
}
