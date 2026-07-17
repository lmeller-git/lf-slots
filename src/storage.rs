use crate::sync::atomic::AtomicBool;

pub(crate) struct InlineStorage<const N: usize> {
    arr: [AtomicBool; N],
}

impl<const N: usize> InlineStorage<N> {
    pub(crate) fn new() -> Self {
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

impl<const N: usize> Default for InlineStorage<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "alloc")]
pub(crate) struct HeapStorage {
    #[allow(unused_qualifications)]
    arr: alloc::boxed::Box<[AtomicBool]>,
}

#[cfg(feature = "alloc")]
impl HeapStorage {
    pub(crate) fn new(size: usize) -> Self {
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

pub(crate) trait StorageBackend {
    fn as_slice(&self) -> &[AtomicBool];
}
