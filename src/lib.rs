// #![deny(missing_docs)]
#![deny(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)]
#![warn(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(test)]
mod tests;

use crate::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "alloc")]
extern crate alloc;

mod sync;

struct InlineStorage<const N: usize> {
    arr: [AtomicBool; N],
}

impl<const N: usize> InlineStorage<N> {
    fn new() -> Self {
        Self {
            arr: core::array::from_fn(|_| AtomicBool::new(true)),
        }
    }
}

impl<const N: usize> StorageBackend for InlineStorage<N> {
    fn as_slice(&self) -> &[AtomicBool] {
        self.arr.as_ref()
    }
}

#[cfg(feature = "alloc")]
struct HeapStorage {
    arr: alloc::boxed::Box<[AtomicBool]>,
}

impl HeapStorage {
    fn new(size: usize) -> Self {
        Self {
            arr: (0..size).map(|_| AtomicBool::new(true)).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl StorageBackend for HeapStorage {
    fn as_slice(&self) -> &[AtomicBool] {
        self.arr.as_ref()
    }
}

trait StorageBackend {
    fn as_slice(&self) -> &[AtomicBool];
}

pub(crate) struct Storage<S> {
    storage: S,
}

impl<S: StorageBackend> Storage<S> {
    pub(crate) fn new(backend: S) -> Self {
        Self { storage: backend }
    }

    pub(crate) fn pull(&self) -> Option<usize> {
        let store = self.storage.as_slice();
        for (idx, item) in store.iter().enumerate() {
            if item
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return Some(idx);
            }
        }

        None
    }

    pub(crate) fn put(&self, index: usize) -> bool {
        self.storage
            .as_slice()
            .get(index)
            .map(|slot| slot.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed))
            .is_some_and(|r| r.is_ok())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.storage
            .as_slice()
            .iter()
            .all(|item| item.load(Ordering::Acquire))
    }

    pub(crate) fn is_full(&self) -> bool {
        self.storage
            .as_slice()
            .iter()
            .all(|item| !item.load(Ordering::Acquire))
    }

    pub(crate) fn len(&self) -> usize {
        self.storage
            .as_slice()
            .iter()
            .map(|item| !item.load(Ordering::Acquire) as usize)
            .sum()
    }

    pub(crate) fn capacity(&self) -> usize {
        self.storage.as_slice().len()
    }
}

pub struct InlineSlots<const N: usize>(Storage<InlineStorage<N>>);

impl<const N: usize> InlineSlots<N> {
    pub fn new() -> Self {
        Self(Storage::new(InlineStorage::new()))
    }

    pub fn pull(&self) -> Option<usize> {
        self.0.pull()
    }

    pub fn put(&self, index: usize) -> bool {
        self.0.put(index)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

#[cfg(feature = "alloc")]
pub struct HeapSlots(Storage<HeapStorage>);

#[cfg(feature = "alloc")]
impl HeapSlots {
    pub fn new(size: usize) -> Self {
        Self(Storage::new(HeapStorage::new(size)))
    }

    pub fn pull(&self) -> Option<usize> {
        self.0.pull()
    }

    pub fn put(&self, index: usize) -> bool {
        self.0.put(index)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}
