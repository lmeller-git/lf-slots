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

    fn raw_words(&self) -> &[AtomicWord];
}

const _: () = assert!(
    BITS_PER_CACHE_LINE.is_power_of_two(),
    "BITS_PER_CACHE_LINE must be a power of two for bitwise math to work"
);

pub(crate) struct BitsetStorage<const WORDS: usize = WORDS_PER_CACHE_LINE> {
    words: CachePadded<[AtomicWord; WORDS]>,
}

impl<const WORDS: usize> BitsetStorage<WORDS> {
    fn free_count(&self) -> usize {
        const { assert!(WORDS.is_power_of_two()) };
        self.words
            .iter()
            .map(|w| w.load(Ordering::Acquire).count_ones() as usize)
            .sum()
    }
}

impl<const WORDS: usize> Default for BitsetStorage<WORDS> {
    fn default() -> Self {
        const { assert!(WORDS.is_power_of_two()) };
        Self {
            words: core::array::from_fn(|_| AtomicWord::new(Word::MAX)).into(),
        }
    }
}

impl<const WORDS: usize> RawSlotPool for BitsetStorage<WORDS> {
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
        _ = unsafe { self.words.get_unchecked(batch.starting_idx / WORD_BITS) }
            .fetch_or(batch.mask, Ordering::Release);
        true
    }
}

impl<const WORDS: usize> SlotPoolMeta for BitsetStorage<WORDS> {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn is_full(&self) -> bool {
        self.len() == Self::SHARD_BITS
    }

    fn len(&self) -> usize {
        self.free_count()
    }

    fn capacity(&self) -> usize {
        Self::SHARD_BITS
    }
}

impl<const WORDS: usize> ShardStorage for BitsetStorage<WORDS> {
    const SHARD_BITS: usize = WORDS * WORD_BITS;
    const SHARD_MASK: usize = Self::SHARD_BITS - 1;
    const SHARD_SHIFT: u32 = Self::SHARD_BITS.ilog2();

    fn raw_words(&self) -> &[AtomicWord] {
        self.words.as_ref()
    }
}

pub(crate) const TARGET_SHARDS: usize = 8;

/// Computes the optimal number of words per shard to balance shard count with memory consumption.
/// Returns between 1 and `WORDS_PER_CACHE_LINE`, targetting at least `TARGET_SHARDS` shards per pool.
pub const fn words_per_shard(capacity: usize) -> usize {
    if capacity == 0 {
        return 0;
    }

    let ideal_bits_per_shard = capacity.div_ceil(TARGET_SHARDS);
    let ideal_words = ideal_bits_per_shard.div_ceil(WORD_BITS);
    let words_p2 = ideal_words.next_power_of_two();

    // TODO use clamp once Ord is const
    if words_p2 > WORDS_PER_CACHE_LINE {
        WORDS_PER_CACHE_LINE
    } else if words_p2 < 1 {
        1
    } else {
        words_p2
    }
}

/// Calculates the number of shards used to store `capacity` slots.
pub const fn shard_count(capacity: usize) -> usize {
    let words = words_per_shard(capacity);
    let shard_bits = words * WORD_BITS;
    capacity.div_ceil(shard_bits)
}
