# RFC: Kasino Migration for Linearizable Slot Pools

## Goals

- **Primary**: Make slot pools optionally linearizable by migrating to `kasino` framework
- **Preserve** current performance characteristics under contention (sharding, cache-coherence strategies)
- **Keep** public API surface (`SlotPool`, `SlotPoolMeta`, `BatchedSlotPool`, `InlineSlots`, `Slots`, `WordSlots`) stable
- **Enable** `kasino`'s `Strategy` trait for scheduling instead of custom `CoherenceProvider`

## Non-Goals

- Rewrite `kasino` itself
- Change the public trait hierarchy (`SlotPool`, `BatchedSlotPool`, etc.)
- Introduce breaking changes to `SlotHandle`, `Batch`, `RawBatch`, `ID`
- Implement linearizability proofs (handled by `kasino` if structure is correct)

---

## Client Usage (Target Test)

See `target_test.rs` — demonstrates:
- `InlineSlots` and `Slots` constructed via macros (unchanged)
- `WordPool` / `InlineWordSlots` via `define_inline_wordslots!` (unchanged)
- Strategy selection via type parameter (e.g., `DCBO`, `Hooked<Custom>`)
- `pull`/`put` and `pull_batch`/`put_batch` work identically
- `fork` for multi-threaded access (new, from `kasino` bandit handle)

---

## Public API Sketch

### Traits (unchanged)

```rust
// slot_alloc.rs - unchanged public surface
pub trait SlotPoolMeta { fn len(&self) -> usize; fn capacity(&self) -> usize; ... }
pub trait SlotPool: RawSlotPool { fn id(&self) -> ID; fn pull(&self) -> Option<SlotHandle>; fn put(&self, SlotHandle) -> Result<(), SlotHandle>; }
pub trait RawSlotPool: SlotPoolMeta { fn pull_raw(&self) -> Option<usize>; unsafe fn put_raw(&self, usize) -> bool; }
pub trait BatchedRawSlotPool: RawSlotPool { fn pull_raw_batch(&self) -> Option<RawBatch>; unsafe fn put_raw_batch(&self, RawBatch) -> bool; ... }
pub trait BatchedSlotPool: BatchedRawSlotPool + SlotPool { fn pull_batch(&self) -> Option<Batch>; fn put_batch(&self, Batch) -> Result<(), Batch>; ... }
```

### Storage Types (signatures unchanged, internals rewritten)

```rust
// storage.rs
pub struct InlineSlots<const N: usize, const SHARDS: usize, const WORDS: usize = WORDS_PER_CACHE_LINE, S = AutoStrategy> { ... }
pub struct Slots<S = AutoStrategy> { ... }  // #[cfg(feature = "alloc")]
pub mod batched {
    #[repr(transparent)]
    pub struct WordPool<P> { inner: P }
    #[cfg(feature = "alloc")]
    pub type WordSlots<S = AutoStrategy> = WordPool<Slots<S>>;
}
```

### Coherence → Strategy Migration

**Old**: `CoherenceProvider` trait with `current_hint()` + `advance_hint_by(count)`

**New**: `kasino::Strategy<Collection>` — use kasino's built-in strategies (`DCBO`, etc.) directly. No `advance_hint_by` equivalent for now (can be added later if needed).

### Macros (unchanged)

```rust
define_inline_slots!(SlotPool42, 42);           // InlineSlots
define_inline_slots!(SlotPool128_1, 128, 1);    // explicit words/shard
define_inline_wordslots!(WordPool42, 42);       // WordPool<InlineSlots<...>>
```

---

## Guarantees

| Property | Status |
|----------|--------|
| **Safety** | `unsafe` only in `RawSlotPool`/`BatchedRawSlotPool` impls; `SlotPool` safe wrapper unchanged |
| **Ordering/Visibility** | `kasino` provides linearizable `offer`/`poll`; our `Collection` impl uses `AcqRel` on atomics |
| **Progress** | Lock-free (kasino bandit is lock-free); wait-free for single-threaded `pull`/`put` |
| **Error Model** | `pull` → `Option`; `put` → `Result<(), SlotHandle>` (ID mismatch / double-free); `pull_batch`/`put_batch` analogous |
| **Send/Sync** | `InlineSlots`: `Send + Sync` when `S: Send + Sync`; `Slots`: same; `WordPool<P>`: same as `P` |

---

## Internal Structure

### 1. Two `Collection` Implementations per Shard Type

Each shard (`BitsetStorage<WORDS>`) implements `Collection` **twice** — once for single-slot operations, once for batched:

```rust
// Single-slot Collection
struct SingleSlotCollection<const WORDS: usize> {
    storage: BitsetStorage<WORDS>,
}

impl<const WORDS: usize> Collection for SingleSlotCollection<WORDS> {
    type OfferSignature = PullSingle;   // Input = usize, ErrorOutput = usize (slot index)
    type PollSignature = PutSingle;     // Input = (), Output = usize

    fn offer(&self, _input: ()) -> Result<usize, ()> {
        Err(input) // always fail, routes to on_offer_fail with the index
    }

    fn poll(&self, input: usize) -> Result<(), usize> {
        self.storage.pull_raw().ok_or(())
    }

    fn len(&self) -> usize { self.storage.len() }
    fn capacity(&self) -> usize { Self::SHARD_BITS }
    fn is_empty(&self) -> bool { self.storage.len() == 0 }
}

// Batched Collection
struct BatchedCollection<const WORDS: usize> {
    storage: BitsetStorage<WORDS>,
}

impl<const WORDS: usize> Collection for BatchedCollection<WORDS> {
    type OfferSignature = PullBatch;    // Input = RawBatch, ErrorOutput = RawBatch
    type PollSignature = PutBatch;      // Input = (), Output = RawBatch

    fn offer(&self, _input: ()) -> Result<RawBatch, ()> {
        Err(input) // always fail, routes to on_offer_fail with the batch
    }

    fn poll(&self, input: RawBatch) -> Result<(), RawBatch> {
        self.storage.pull_raw_batch().ok_or(())
    }

    fn len(&self) -> usize { self.storage.len() }
    fn capacity(&self) -> usize { Self::SHARD_BITS }
    fn is_empty(&self) -> bool { self.storage.len() == 0 }
}
```

**Why two collections**: `offer` returns `Err(input)` to preserve the input for `on_offer_fail`. Single and batched have different input/output types, so they need separate `Collection` impls. The pool chooses which collection to use based on whether `pull` or `pull_batch` is called.

### 2. `InlineSlots` / `Slots` → `InlineBandit` / `Bandit` over Two Collections

```rust
// InlineSlots: stack-allocated array of shards
pub struct InlineSlots<const N, const SHARDS, const WORDS, S> {
    single_bandit: InlineBandit<SingleSlotCollection<WORDS>, S, SHARDS, WORDS>,
    batch_bandit: InlineBandit<BatchedCollection<WORDS>, S, SHARDS, WORDS>,
    id: ID,
    capacity: usize,
}

// Slots: heap-allocated vec of shards
#[cfg(feature = "alloc")]
pub struct Slots<S> {
    single_bandit: Bandit<SingleSlotCollection<WORDS_PER_CACHE_LINE>, S>,
    batch_bandit: Bandit<BatchedCollection<WORDS_PER_CACHE_LINE>, S>,
    id: ID,
    capacity: usize,
}
```

### 3. Strategy — Use Kasino's Built-ins Directly

No `SlotStrategy` adapter. Users pass `S: Strategy<SingleSlotCollection<WORDS>> + Strategy<BatchedCollection<WORDS>>` directly (e.g., `DCBO`, `Hooked<Custom>`). `AutoStrategy` type alias picks a default.

```rust
// Default strategy selection (cfg-gated like AutoCoherenceProvider)
#[cfg(all(feature = "std", not(test), not(loom), not(shuttle)))]
pub type AutoStrategy = DCBO;  // or kasino's default

#[cfg(all(not(feature = "std"), not(test), not(loom), not(shuttle)))]
pub type AutoStrategy = kasino::strategy::DefaultStrategy; // no_std compatible

#[cfg(any(test, loom, shuttle))]
pub type AutoStrategy = kasino::strategy::NoOpStrategy; // deterministic for testing
```

### 4. `on_offer_fail` — Exact Shard Routing for Put

For **both** collections, `offer` always returns `Err(input)`. The strategy's `on_offer_fail` receives the input (slot index or `RawBatch`), computes the exact shard, and delegates to that shard's `offer` with a "put" variant.

```rust
// Strategy impl for SingleSlotCollection
impl<S: Strategy<SingleSlotCollection<WORDS>>, const WORDS: usize> Strategy<SingleSlotCollection<WORDS>> for S {
    type Gambler = S::Gambler;

    fn choose_offer_arm(&self, state, gambler) -> usize { ... }
    fn choose_poll_arm(&self, state, gambler) -> usize { 0 } // unused

    fn on_offer_fail<'b, 'c>(
        &self,
        state: &impl StorageBackend<SingleSlotCollection<WORDS>>,
        sub_collections: &'c impl StorageBackend<SingleSlotCollection<WORDS>>,
        input: usize, // the slot index to put back
        gambler: &mut Self::Gambler,
    ) -> Option<((), usize)> {
        let shard_idx = input >> SHARD_SHIFT;
        let col = sub_collections.get(shard_idx)?;
        // Delegate to that shard's offer with PutInput variant
        // The shard's offer handles PutInput by doing the put and returning Ok(())
        col.offer(PutInput(input)).ok()?;
        Some(((), shard_idx))
    }
}
```

Each shard's `offer` handles both `PullInput(())` and `PutInput(usize)`:
```rust
enum OfferInput { Pull, Put(usize) }

fn offer(&self, input: OfferInput) -> Result<OfferOutput, OfferInput> {
    match input {
        OfferInput::Pull => self.pull_raw().map(OfferOutput::Slot).ok_or(OfferInput::Pull),
        OfferInput::Put(idx) => {
            unsafe { self.put_raw(idx) };
            Ok(OfferOutput::PutDone)
        }
    }
}
```

**Same pattern for batched**: `PollSignature::Input = RawBatch`, `on_offer_fail` extracts `starting_idx`, computes shard, delegates to that shard's `offer(PutBatch(batch))`.

### 5. `WordPool<P>` — Unchanged Wrapper

Delegates to inner `P: BatchedSlotPool`. Works with `InlineBandit<BatchedCollection, ...>` directly.

---

## Migration Analysis

### a) Types to Keep vs Rewrite

| Type | Action |
|------|--------|
| `SlotPoolMeta`, `SlotPool`, `RawSlotPool`, `BatchedRawSlotPool`, `BatchedSlotPool` | **Keep** — public API |
| `SlotHandle`, `Batch`, `RawBatch`, `ID`, `Word` | **Keep** — core types |
| `define_inline_slots!`, `define_inline_wordslots!` | **Keep** — macros compute layout |
| `CoherenceProvider` + impls | **Remove** — replaced by kasino `Strategy` |
| `BitsetStorage` | **Keep** as core shard; wrap in two `Collection` impls |
| `GenericStorage` | **Remove** — replaced by `InlineBandit`/`Bandit` |
| `InlineBuffer`, `HeapBuf` | **Remove** — `kasino` provides storage backends |
| `InlineSlots`, `Slots` | **Rewrite** internals (two bandits); public API unchanged |
| `WordPool` | **Keep** — thin wrapper |
| `bitshard::words_per_shard`, `shard_count` | **Keep** — used by macros |
| `cache_coherence` module | **Remove** — replace with `strategy` re-exports |

### b) WordSlots/Batched API: Shard Directly Over `BitsetStorage`

**Direct sharding over `BitsetStorage` (preferred)** — same as before.

- `BatchedCollection<WORDS>` implements `Collection` with `OfferOutput = RawBatch`
- `WordPool<InlineBandit<BatchedCollection, ...>>` works directly
- Shard count: `words_per_shard(capacity * WORD_BITS)`

### c) Scheduler → Strategy: Use Kasino Built-ins

**No custom `advance_hint_by` migration**. Drop the old schedulers. Use kasino's `DCBO`, `RoundRobin`, etc. directly. If hint-advancement behavior is needed later, it can be added via a custom `Strategy` implementation or upstream kasino changes.

| Old `CoherenceProvider` | Replacement |
|-------------------------|-------------|
| `NoCoherence` | `kasino::strategy::NoOpStrategy` |
| `ThreadLocalRoundRobin` | `kasino::strategy::RoundRobin` (or `Hooked` with thread-local state) |
| `StripedRoundRobin` | `kasino::strategy::RoundRobin` with striped gambler |
| `AutoCoherenceProvider` | `AutoStrategy` type alias (cfg-gated) |

### d) API & Performance Impact

| Aspect | Impact |
|--------|--------|
| **Public API** | Zero breaking changes. `C: CoherenceProvider` → `S: Strategy` (same type param pattern). |
| **Construction** | `InlineSlots::with_coherence_provider::<C>()` → `InlineSlots::with_strategy::<S>()`. |
| **Performance** | Same sharding, same cache-line layout. Two bandits (single + batch) per pool — negligible overhead. |
| **Linearizability** | Opt-in via `Strategy` (e.g., `DCBO`). `NoOpStrategy` → non-linearizable (current behavior). |
| **Feature Flags** | Unchanged. `kasino` features may be needed. |

---

## Open Questions

### 1. Two Collections per Pool (Single + Batched)

**Problem**: `pull` and `pull_batch` need different `Collection` types because their `OfferSignature` differs (`usize` vs `RawBatch`).

**Resolution**: Two bandits per pool (`single_bandit`, `batch_bandit`). Each wraps the same `BitsetStorage` shards but with different `Collection` impls. The shards themselves are shared (same `BitsetStorage` instances). This is clean and type-safe.

**Tradeoff**: Slightly more memory (two bandit handles). Acceptable.

### 2. `offer` Returns `Err(input)` for Put Routing

**Problem**: `put` needs the exact index/batch to know which shard to write to. `kasino`'s `poll` doesn't receive this input by default.

**Resolution**: `poll` signature takes `Input` (index or `RawBatch`) and **always returns `Err(input)`**. `on_offer_fail` receives the `Err` payload, computes shard, delegates to that shard's `offer` with a "put" variant. This mirrors `sharded_rwlock`'s pattern.

### 3. No `advance_hint_by` Equivalent

**Problem**: Old schedulers advanced hint on **successful pull** to dodge incoming puts. Kasino's `on_offer_fail` runs on **failed offer** (retry), not success.

**Resolution**: **Drop it for now**. Use kasino's built-in strategies. If benchmarking shows hint-advancement matters, add a custom `Strategy` later that tracks last successful arm in `Gambler` and advances in `choose_offer_arm`. This avoids kasino changes.

### 4. `WordPool` with Kasino Bandit

**Resolution**: `BatchedCollection` returns `RawBatch` from `offer`. `InlineBandit<BatchedCollection, ...>` implements `BatchedSlotPool` by delegating `pull_batch` → `offer`, `put_batch` → `poll` (which routes via `on_offer_fail`). `WordPool` works unchanged.

### 5. `atomic-fallback` Feature

**Resolution**: Keep as-is. `BitsetStorage` uses `portable-atomic`; `kasino` uses its own atomics (also `portable-atomic` via deps). No extra work.

---

## Migration Phases

1. **Phase 1**: Add `kasino` dependency. Implement `SingleSlotCollection` and `BatchedCollection` for `BitsetStorage`. Define `AutoStrategy` type alias.
2. **Phase 2**: Rewrite `InlineSlots` / `Slots` with two bandits (`single_bandit`, `batch_bandit`). Remove `GenericStorage`, `InlineBuffer`, `HeapBuf`, `cache_coherence` module.
3. **Phase 3**: Implement `SlotPool`/`BatchedSlotPool` for bandit-wrapped types. Wire `pull` → `single_bandit.offer`, `pull_batch` → `batch_bandit.offer`, `put`/`put_batch` → `poll` (routes via `on_offer_fail`).
4. **Phase 4**: Verify `WordPool` works. Update macros for bandit-compatible `SHARDS`/`WORDS`.
5. **Phase 5**: Run full test suite (miri, loom, shuttle, benchmarks). Compare performance.
6. **Phase 6**: Export `strategy` module re-exporting kasino strategies. Document custom `Strategy` usage.

---

## References

- `kasino-examples/sharded_rwlock.rs` — pattern for `Collection` + `Strategy` + `on_offer_fail` routing
- `kasino` crate docs: `Collection`, `Strategy`, `InlineBandit`, `Bandit`, `StorageBackend`
