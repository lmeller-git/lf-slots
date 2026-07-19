use crossbeam_utils::CachePadded;

#[cfg(feature = "alloc")]
use crate::slot_alloc::StorageExt;
use crate::{
    slot_alloc::{RawStorage, SlotHandle, next_id},
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

// TODO 32 bit atomcis under cfg

const WORD_BITS: usize = u64::BITS as usize;

pub(crate) struct BitsetStorage<const WORDS: usize> {
    words: CachePadded<[AtomicU64; WORDS]>,
}

impl<const WORDS: usize> BitsetStorage<WORDS> {
    pub(crate) const BITS: usize = WORDS * WORD_BITS;

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

pub(crate) struct MaskedBitsetStorage<const WORDS: usize> {
    inner: BitsetStorage<WORDS>,
    usable: u32,
}

impl<const WORDS: usize> MaskedBitsetStorage<WORDS> {
    pub(crate) fn new(usable: usize) -> Self {
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

pub(crate) struct InlineStorage<T, const N: usize> {
    arr: [T; N],
    cursor: AtomicUsize,
    id: u64,
}

impl<T: Default, const N: usize> InlineStorage<T, N> {
    pub(crate) fn new() -> Self {
        Self {
            arr: core::array::from_fn(|_| T::default()),
            cursor: AtomicUsize::new(0),
            id: next_id(),
        }
    }
}

impl<T: Default, const N: usize> Default for InlineStorage<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: RawStorage, const N: usize> RawStorage for InlineStorage<T, N> {
    fn pull_raw(&self) -> Option<usize> {
        if N == 0 {
            return None;
        }
        let start = self.cursor.fetch_add(1, Ordering::Relaxed) % N;
        for offset in 0..N {
            let idx = (start + offset) % N;
            let item = &self.arr[idx];
            if let Some(inner_idx) = item.pull_raw() {
                return Some(inner_idx + idx * item.capacity());
            }
        }
        None
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        if self.arr.is_empty() {
            return false;
        }
        let inner_capacity = self.arr[0].capacity();
        if inner_capacity == 0 {
            return false;
        }
        let row = index / inner_capacity;
        let col = index % inner_capacity;
        self.arr
            .get(row)
            .map(|slot| unsafe { slot.put_raw(col) })
            .unwrap_or(false)
    }

    fn is_empty(&self) -> bool {
        self.arr.iter().all(|slot| slot.is_empty())
    }

    fn is_full(&self) -> bool {
        self.arr.iter().all(|slot| slot.is_full())
    }

    fn len(&self) -> usize {
        self.arr.iter().map(|slot| slot.len()).sum()
    }

    fn capacity(&self) -> usize {
        self.arr.iter().map(|slot| slot.capacity()).sum()
    }
}

impl<T: RawStorage, const N: usize> StorageExt for InlineStorage<T, N> {
    fn pull(&self) -> Option<SlotHandle> {
        self.pull_raw().map(|raw| SlotHandle::new(raw, self.id))
    }

    fn put(&self, index: SlotHandle) -> bool {
        if index.id() != self.id {
            return false;
        }
        unsafe { self.put_raw(index.as_usize()) }
    }
}

#[cfg(feature = "alloc")]
pub(crate) struct HeapStorage<T> {
    #[allow(unused_qualifications)]
    arr: alloc::boxed::Box<[T]>,
    cursor: AtomicUsize,
    id: u64,
}

#[cfg(feature = "alloc")]
impl<T: Default> HeapStorage<T> {
    pub(crate) fn new(size: usize) -> Self {
        Self {
            arr: (0..size).map(|_| T::default()).collect(),
            cursor: AtomicUsize::new(0),
            id: next_id(),
        }
    }
}

#[cfg(feature = "alloc")]
impl<T: RawStorage> RawStorage for HeapStorage<T> {
    fn pull_raw(&self) -> Option<usize> {
        let n = self.arr.len();
        if n == 0 {
            return None;
        }
        let start = self.cursor.fetch_add(1, Ordering::Relaxed) % n;
        for offset in 0..n {
            let idx = (start + offset) % n;
            let item = &self.arr[idx];
            if let Some(inner_idx) = item.pull_raw() {
                return Some(inner_idx + idx * item.capacity());
            }
        }
        None
    }

    unsafe fn put_raw(&self, index: usize) -> bool {
        if self.arr.is_empty() {
            return false;
        }
        let inner_capacity = self.arr[0].capacity();
        if inner_capacity == 0 {
            return false;
        }
        let row = index / inner_capacity;
        let col = index % inner_capacity;
        self.arr
            .get(row)
            .map(|slot| unsafe { slot.put_raw(col) })
            .unwrap_or(false)
    }

    fn is_empty(&self) -> bool {
        self.arr.iter().all(|slot| slot.is_empty())
    }

    fn is_full(&self) -> bool {
        self.arr.iter().all(|slot| slot.is_full())
    }

    fn len(&self) -> usize {
        self.arr.iter().map(|slot| slot.len()).sum()
    }

    fn capacity(&self) -> usize {
        self.arr.iter().map(|slot| slot.capacity()).sum()
    }
}

#[cfg(feature = "alloc")]
impl<T: RawStorage> StorageExt for HeapStorage<T> {
    fn pull(&self) -> Option<SlotHandle> {
        self.pull_raw().map(|raw| SlotHandle::new(raw, self.id))
    }

    fn put(&self, index: SlotHandle) -> bool {
        if index.id() != self.id {
            return false;
        }
        unsafe { self.put_raw(index.as_usize()) }
    }
}

pub const fn full_shard_count(n: usize, shard_bits: usize) -> usize {
    n / shard_bits
}

pub const fn tail_bits(n: usize, shard_bits: usize) -> usize {
    n % shard_bits
}

#[macro_export]
macro_rules! define_index_store {
    ($name:ident, $ctor:ident, $n:expr) => {
        pub(crate) type $name = $crate::ConcatStorage<
            $crate::InlineStorage<$crate::BitsetStorage<8>, { $crate::full_shard_count($n, 512) }>,
            $crate::MaskedBitsetStorage<8>,
        >;

        pub(crate) fn $ctor() -> $name {
            $crate::ConcatStorage::new(
                ::core::default::Default::default(),
                $crate::MaskedBitsetStorage::new($crate::tail_bits($n, 512)),
            )
        }
    };
}
