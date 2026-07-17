pub trait SlotStorage {
    fn pull(&self) -> Option<usize>;
    fn put(&self, index: usize) -> bool;
    fn is_empty(&self) -> bool;
    fn is_full(&self) -> bool;
    fn len(&self) -> usize;
    fn capacity(&self) -> usize;
}
