//! Target test for kasino-migration RFC
//! This test demonstrates the desired end-state client API after migration.
//! It will not compile until the migration is complete.

use lf_slots::{SlotPool, SlotPoolMeta, BatchedSlotPool, define_inline_slots, define_inline_wordslots};
#[cfg(feature = "alloc")]
use lf_slots::{Slots, BatchedSlotPool};
#[cfg(feature = "word-slots")]
use lf_slots::batched::WordPool;
use kasino::strategy::DCBO;

fn main() {
    // ===== InlineSlots (stack-allocated) =====
    define_inline_slots!(SlotPool42, 42);
    let pool = SlotPool42::<DCBO>::new();

    assert_eq!(pool.capacity(), 42);
    assert_eq!(pool.len(), 42);

    let handle = pool.pull().unwrap();
    assert_eq!(pool.len(), 41);
    let idx = handle.as_usize();
    assert!(pool.put(handle).is_ok());
    assert!(pool.is_full());

    // Batched operations
    let batch = pool.pull_batch().unwrap();
    assert!(batch.count() > 0);
    assert!(pool.put_batch(batch).is_ok());

    // ===== Slots (heap-allocated, needs alloc) =====
    #[cfg(feature = "alloc")]
    {
        let pool = Slots::<DCBO>::new(128);
        assert_eq!(pool.capacity(), 128);
        assert_eq!(pool.len(), 128);

        let handle = pool.pull().unwrap();
        assert_eq!(pool.len(), 127);
        assert!(pool.put(handle).is_ok());
        assert!(pool.is_full());
    }

    // ===== WordSlots / InlineWordSlots (bit-packed, word-slots feature) =====
    #[cfg(feature = "word-slots")]
    {
        define_inline_wordslots!(WordPool42, 42);
        let pool = WordPool42::<DCBO>::new();

        // Capacity is in words (each word = WORD_BITS slots)
        assert_eq!(pool.capacity(), 42);
        assert_eq!(pool.len(), 42);

        let handle = pool.pull().unwrap();
        assert_eq!(pool.len(), 41);
        assert!(pool.put(handle).is_ok());
        assert!(pool.is_full());

        // Batched operations on word pools pull/put whole words
        let batch = pool.pull_batch().unwrap();
        assert_eq!(batch.count(), 1); // WordPool batches are always 1 word
        assert!(pool.put_batch(batch).is_ok());
    }

    #[cfg(all(feature = "word-slots", feature = "alloc"))]
    {
        let pool = WordPool::<Slots<DCBO>>::new(64); // 64 words = 64 * WORD_BITS slots
        assert_eq!(pool.capacity(), 64);
        let handle = pool.pull().unwrap();
        assert!(pool.put(handle).is_ok());
    }

    // ===== Custom Strategy =====
    // Users can plug in their own kasino::Strategy implementation
    // struct MyStrategy;
    // impl<Col> kasino::Strategy<Col> for MyStrategy { ... }
    // define_inline_slots!(MyPool, 100);
    // let pool = MyPool::<MyStrategy>::new();

    // ===== Forking for Multi-threaded Access =====
    // kasino bandits provide forkable handles for concurrent access
    // let pool = SlotPool42::<DCBO>::new();
    // let mut handle1 = pool.handle();
    // let mut handle2 = handle1.fork();
    // std::thread::scope(|s| {
    //     s.spawn(|| { let _ = handle1.pull(); });
    //     s.spawn(|| { let _ = handle2.pull(); });
    // });

    println!("All target test scenarios compiled successfully!");
}