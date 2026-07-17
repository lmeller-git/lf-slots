use crate::{storage::StorageBackend, sync::atomic::Ordering};

pub trait SlotStorage {
    fn pull(&self) -> Option<usize>;
    fn put(&self, index: usize) -> bool;
    fn is_empty(&self) -> bool;
    fn is_full(&self) -> bool;
    fn len(&self) -> usize;
    fn capacity(&self) -> usize;
}

pub(crate) struct Storage<S> {
    storage: S,
}

impl<S: StorageBackend> Storage<S> {
    pub(crate) fn new(backend: S) -> Self {
        Self { storage: backend }
    }
}

impl<S: StorageBackend> SlotStorage for Storage<S> {
    fn pull(&self) -> Option<usize> {
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

    fn put(&self, index: usize) -> bool {
        self.storage
            .as_slice()
            .get(index)
            .map(|slot| slot.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed))
            .is_some_and(|r| r.is_ok())
    }

    fn is_empty(&self) -> bool {
        self.storage
            .as_slice()
            .iter()
            .all(|item| item.load(Ordering::Acquire))
    }

    fn is_full(&self) -> bool {
        self.storage
            .as_slice()
            .iter()
            .all(|item| !item.load(Ordering::Acquire))
    }

    fn len(&self) -> usize {
        self.storage
            .as_slice()
            .iter()
            .map(|item| !item.load(Ordering::Acquire) as usize)
            .sum()
    }

    fn capacity(&self) -> usize {
        self.storage.as_slice().len()
    }
}
