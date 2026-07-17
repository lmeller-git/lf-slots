use crate::{
    slot_alloc::SlotStorage,
    sync::atomic::{AtomicBool, Ordering},
};

pub(crate) struct BooleanStorage(AtomicBool);

impl Default for BooleanStorage {
    fn default() -> Self {
        Self(AtomicBool::new(true))
    }
}

impl SlotStorage for BooleanStorage {
    fn pull(&self) -> Option<usize> {
        self.0
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
            .ok()
            .map(|_| 0)
    }

    fn put(&self, _index: usize) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    fn is_empty(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    fn is_full(&self) -> bool {
        !self.is_empty()
    }

    fn len(&self) -> usize {
        self.is_full() as usize
    }

    fn capacity(&self) -> usize {
        1
    }
}

pub(crate) struct InlineStorage<T, const N: usize> {
    arr: [T; N],
}

impl<T: Default, const N: usize> InlineStorage<T, N> {
    pub(crate) fn new() -> Self {
        Self {
            arr: core::array::from_fn(|_| T::default()),
        }
    }
}

impl<T: Default, const N: usize> Default for InlineStorage<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: SlotStorage, const N: usize> SlotStorage for InlineStorage<T, N> {
    fn pull(&self) -> Option<usize> {
        for (idx, item) in self.arr.iter().enumerate() {
            if let Some(inner_idx) = item.pull() {
                return Some(inner_idx + idx * item.capacity());
            }
        }
        None
    }

    fn put(&self, index: usize) -> bool {
        if self.arr.is_empty() {
            return false;
        }

        let inner_capacity = self.arr[0].capacity();
        if inner_capacity == 0 {
            return false;
        }

        let row = index / inner_capacity;
        let col = index % inner_capacity;

        self.arr.get(row).map(|slot| slot.put(col)).unwrap_or(false)
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
pub(crate) struct HeapStorage<T> {
    #[allow(unused_qualifications)]
    arr: alloc::boxed::Box<[T]>,
}

#[cfg(feature = "alloc")]
impl<T: Default> HeapStorage<T> {
    pub(crate) fn new(size: usize) -> Self {
        Self {
            arr: (0..size).map(|_| T::default()).collect(),
        }
    }
}

#[cfg(feature = "alloc")]
impl<T: SlotStorage> SlotStorage for HeapStorage<T> {
    fn pull(&self) -> Option<usize> {
        for (idx, item) in self.arr.iter().enumerate() {
            if let Some(inner_idx) = item.pull() {
                return Some(inner_idx + idx * item.capacity());
            }
        }
        None
    }

    fn put(&self, index: usize) -> bool {
        if self.arr.is_empty() {
            return false;
        }

        let inner_capacity = self.arr[0].capacity();
        if inner_capacity == 0 {
            return false;
        }

        let row = index / inner_capacity;
        let col = index % inner_capacity;

        self.arr.get(row).map(|slot| slot.put(col)).unwrap_or(false)
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
