use crate::InlineSlots;

#[test]
fn basic() {
    let storage: InlineSlots<10> = InlineSlots::new();
    assert_eq!(storage.capacity(), 10);

    assert!(storage.is_empty());
    for _ in 0..10 {
        assert!(storage.pull().is_some());
    }
    assert!(storage.is_full());
    assert_eq!(storage.len(), 10);

    assert!(storage.put(5));
    assert!(!storage.is_full());

    assert_eq!(storage.len(), 9);

    assert_eq!(storage.pull(), Some(5));

    for i in 0..10 {
        assert!(storage.put(i));
    }

    assert!(storage.is_empty());
    assert_eq!(storage.len(), 0);

    assert_eq!(storage.pull(), Some(0));
}
