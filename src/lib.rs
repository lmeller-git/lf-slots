// #![deny(missing_docs)]
#![deny(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)]
#![warn(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(test, feature = "std"))]
mod tests;

mod slot_alloc;
mod storage;
mod sync;

#[cfg(feature = "alloc")]
use crate::storage::HeapStorage;
use crate::{
    slot_alloc::{SlotStorage, Storage},
    storage::InlineStorage,
};

pub struct InlineSlots<const N: usize>(Storage<InlineStorage<N>>);

impl<const N: usize> InlineSlots<N> {
    pub fn new() -> Self {
        Self(Storage::new(InlineStorage::new()))
    }
}

impl<const N: usize> Default for InlineSlots<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "alloc")]
pub struct HeapSlots(Storage<HeapStorage>);

#[cfg(feature = "alloc")]
impl HeapSlots {
    pub fn new(size: usize) -> Self {
        Self(Storage::new(HeapStorage::new(size)))
    }
}

impl<const N: usize> SlotStorage for InlineSlots<N> {
    fn pull(&self) -> Option<usize> {
        self.0.pull()
    }

    fn put(&self, index: usize) -> bool {
        self.0.put(index)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn is_full(&self) -> bool {
        self.0.is_full()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

#[cfg(feature = "alloc")]
impl SlotStorage for HeapSlots {
    fn pull(&self) -> Option<usize> {
        self.0.pull()
    }

    fn put(&self, index: usize) -> bool {
        self.0.put(index)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn is_full(&self) -> bool {
        self.0.is_full()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn capacity(&self) -> usize {
        self.0.capacity()
    }
}
