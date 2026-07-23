use crossbeam_utils::CachePadded;

#[cfg(not(loom))]
use crate::core_internal::WORD_BYTES;
use crate::{
    SlotPoolMeta,
    core::{RawBatch, RawSlotPool},
    core_internal::{AtomicWord, WORD_BITS, Word},
    sync::atomic::Ordering,
};

#[cfg(not(loom))]
#[allow(unused_qualifications)]
pub(crate) const CACHE_LINE_BYTES: usize = core::mem::align_of::<CachePadded<()>>();
#[cfg(not(loom))]
pub(crate) const WORDS_PER_CACHE_LINE: usize = CACHE_LINE_BYTES / WORD_BYTES;
#[cfg(loom)]
pub(crate) const WORDS_PER_CACHE_LINE: usize = 1;
pub(crate) const BITS_PER_CACHE_LINE: usize = WORDS_PER_CACHE_LINE * WORD_BITS;

pub(crate) trait ShardStorage {
    const SHARD_BITS: usize;
    const SHARD_SHIFT: u32;
    const SHARD_MASK: usize;
}

const _: () = assert!(
    BITS_PER_CACHE_LINE.is_power_of_two(),
    "BITS_PER_CACHE_LINE must be a power of two for bitwise math to work!"
);

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

    unsafe fn put_raw(&self, index: usize) -> bool {
        let word_idx = index / WORD_BITS;
        let bit = index % WORD_BITS;
        let mask = 1 << bit;
        // SAFETY:
        // the index is in range of totalbits
        let prev = unsafe { self.words.get_unchecked(word_idx) }.fetch_or(mask, Ordering::Release);
        prev & mask == 0
    }

    fn pull_raw_batch(&self) -> Option<RawBatch> {
        for (word_idx, word) in self.words.iter().enumerate() {
            let mut current = word.load(Ordering::Relaxed);

            while current != 0 {
                match word.compare_exchange_weak(current, 0, Ordering::AcqRel, Ordering::Relaxed) {
                    Ok(_) => {
                        return Some(RawBatch {
                            starting_idx: word_idx * WORD_BITS,
                            mask: current,
                        });
                    }
                    Err(observed) => current = observed,
                }
            }
        }

        None
    }

    unsafe fn put_raw_batch(&self, batch: RawBatch) -> bool {
        // SAFETY:
        // The caller promises that this batch is valid
        _ = unsafe { self.words.get_unchecked(batch.starting_idx) }
            .fetch_or(batch.mask, Ordering::Release);
        true
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

impl ShardStorage for BitsetStorage {
    const SHARD_BITS: usize = BITS_PER_CACHE_LINE;
    const SHARD_MASK: usize = BITS_PER_CACHE_LINE - 1;
    const SHARD_SHIFT: u32 = BITS_PER_CACHE_LINE.ilog2();
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

    unsafe fn put_raw(&self, index: usize) -> bool {
        // SAFETY:
        // The index was returned by self.inner.pull_raw()
        unsafe { self.inner.put_raw(index) }
    }

    fn pull_raw_batch(&self) -> Option<RawBatch> {
        self.inner.pull_raw_batch()
    }

    unsafe fn put_raw_batch(&self, batch: RawBatch) -> bool {
        // SAFETY:
        // the caller promises that this batch is valid
        unsafe { self.inner.put_raw_batch(batch) }
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

impl ShardStorage for MaskedBitsetStorage {
    const SHARD_BITS: usize = BITS_PER_CACHE_LINE;
    const SHARD_MASK: usize = BITS_PER_CACHE_LINE - 1;
    const SHARD_SHIFT: u32 = BITS_PER_CACHE_LINE.ilog2();
}

/// Computes the numer of shards used to store `n` slots
pub const fn full_shard_count(n: usize) -> usize {
    n / BITS_PER_CACHE_LINE
}

/// Computes how many bits in the last shard should stay unused to sotre exactly `n` slots
pub const fn tail_bits(n: usize) -> usize {
    n % BITS_PER_CACHE_LINE
}
