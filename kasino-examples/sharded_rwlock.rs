use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
};

use crossbeam_utils::CachePadded;
use kasino::{
    Collection,
    InlineBandit,
    InlineBanditHandle,
    Signature,
    WithCapacity,
    storage::StorageBackend,
    strategy::{DCBO, Hooked, Strategy, StrategyStakes},
};

#[derive(Debug)]
struct ReaderShard<T>(CachePadded<AtomicUsize>, PhantomData<T>);

impl<T> Default for ReaderShard<T> {
    fn default() -> Self {
        Self(Default::default(), PhantomData)
    }
}

impl<T> WithCapacity<1> for ReaderShard<T> {
    fn with_capacity() -> Self {
        Self(AtomicUsize::new(0).into(), PhantomData)
    }
}

struct LockInput<'a, T> {
    writer: &'a AtomicBool,
    data: NonNull<UnsafeCell<T>>,
}

impl<'a, T> Copy for LockInput<'a, T> {}
impl<'a, T> Clone for LockInput<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

struct ReaderGuard<'a, 'b, T> {
    shard: &'b ReaderShard<T>,
    ptr: NonNull<T>,
    _life: PhantomData<&'a ()>,
}

impl<'a, 'b, T> Deref for ReaderGuard<'a, 'b, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl<'a, 'b, T> Drop for ReaderGuard<'a, 'b, T> {
    fn drop(&mut self) {
        self.shard.0.fetch_sub(1, Ordering::Release);
    }
}

struct ReaderShardOffer<T>(PhantomData<T>);

impl<T> Signature for ReaderShardOffer<T> {
    type Error<'a, 'b>
        = ()
    where
        Self: 'b;
    type Input<'a> = LockInput<'a, T>;
    type Output<'a, 'b>
        = ReaderGuard<'a, 'b, T>
    where
        Self: 'b;
}

#[derive(Debug)]
struct WriteGuard<'a, T> {
    b: &'a AtomicBool,
    ptr: NonNull<T>,
}

impl<'a, T> Deref for WriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl<'a, T> DerefMut for WriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

impl<'a, T> Drop for WriteGuard<'a, T> {
    fn drop(&mut self) {
        self.b.store(false, Ordering::Release);
    }
}

struct WritePoll<T>(PhantomData<T>);

impl<T> Signature for WritePoll<T> {
    type Error<'a, 'b>
        = usize
    where
        Self: 'b;
    type Input<'a> = LockInput<'a, T>;
    type Output<'a, 'b>
        = WriteGuard<'a, T>
    where
        Self: 'b;
}

impl<T> Collection for ReaderShard<T> {
    type OfferSignature = ReaderShardOffer<T>;
    type PollSignature = WritePoll<T>;

    fn offer<'b, 'a>(
        &'b self,
        item: <Self::OfferSignature as Signature>::Input<'a>,
    ) -> Result<
        <Self::OfferSignature as Signature>::Output<'a, 'b>,
        <Self::OfferSignature as Signature>::Error<'a, 'b>,
    > {
        let old_writer = item.writer.load(Ordering::Acquire);
        if old_writer {
            return Err(());
        }
        self.0.fetch_add(1, Ordering::Release);
        let writer_now = item.writer.load(Ordering::Acquire);
        if writer_now {
            self.0.fetch_sub(1, Ordering::Release);
            Err(())
        } else {
            Ok(ReaderGuard {
                shard: self,
                ptr: item.data.cast(),
                _life: PhantomData,
            })
        }
    }

    fn poll<'a, 'b>(
        &'b self,
        _input: <Self::PollSignature as Signature>::Input<'a>,
    ) -> Result<
        <Self::PollSignature as Signature>::Output<'a, 'b>,
        <Self::PollSignature as Signature>::Error<'a, 'b>,
    > {
        Err(self.0.load(Ordering::Acquire))
    }

    fn len(&self) -> usize {
        1
    }

    fn capacity(&self) -> usize {
        1
    }

    fn is_empty(&self) -> bool {
        self.0.load(Ordering::Acquire) == 0
    }
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
struct RwLockStrategy<S>(S);

impl<T, S: Strategy<ReaderShard<T>>> Strategy<ReaderShard<T>> for RwLockStrategy<S> {
    type Gambler = S::Gambler;

    fn choose_offer_arm(
        &self,
        state: &impl StorageBackend<<Self::Gambler as Hooked>::Stake>,
        arm: &mut Self::Gambler,
    ) -> usize {
        self.0.choose_offer_arm(state, arm)
    }

    fn choose_poll_arm(
        &self,
        state: &impl StorageBackend<<Self::Gambler as Hooked>::Stake>,
        arm: &mut Self::Gambler,
    ) -> usize {
        self.0.choose_poll_arm(state, arm)
    }

    fn fork_gambler(&self, arm: &Self::Gambler) -> Self::Gambler {
        self.0.fork_gambler(arm)
    }

    fn create_gambler(&self) -> Self::Gambler {
        self.0.create_gambler()
    }

    fn on_poll_fail<'b, 'c>(
        &self,
        _state: &impl StorageBackend<<Self::Gambler as Hooked>::Stake>,
        sub_collections: &'c impl StorageBackend<ReaderShard<T>>,
        input: <<ReaderShard<T> as Collection>::PollSignature as Signature>::Input<'b>,
        _gambler: &mut Self::Gambler,
    ) -> Option<(
        <<ReaderShard<T> as Collection>::PollSignature as Signature>::Output<'b, 'c>,
        usize,
    )>
    where
        ReaderShard<T>: 'c,
    {
        fn no_reader<'b, 'c, T>(
            sub_collections: &'c impl StorageBackend<ReaderShard<T>>,
            input: LockInput<'b, T>,
        ) -> bool {
            sub_collections
                .iter()
                .all(|item| matches!(item.poll(input), Err(0)))
        }

        if !no_reader(sub_collections, input) {
            return None;
        }

        if input.writer.swap(true, Ordering::AcqRel) {
            return None;
        }

        if !no_reader(sub_collections, input) {
            input.writer.store(false, Ordering::Release);
            return None;
        }

        Some((
            WriteGuard {
                b: input.writer,
                ptr: input.data.cast(),
            },
            0,
        ))
    }
}

struct ShardedRwLock<T, S: Strategy<ReaderShard<T>>, const N: usize> {
    shards: InlineBandit<ReaderShard<T>, RwLockStrategy<S>, N, 1>,
    writer: AtomicBool,
    item: UnsafeCell<T>,
}

impl<T, S, const N: usize> ShardedRwLock<T, S, N>
where
    S: Strategy<ReaderShard<T>> + Default,
    StrategyStakes<RwLockStrategy<S>, ReaderShard<T>>: Default,
{
    fn new(item: T) -> Self {
        Self {
            shards: InlineBandit::new(),
            writer: AtomicBool::new(false),
            item: UnsafeCell::new(item),
        }
    }

    fn new_root(&self) -> ShardedRwLockHandle<'_, T, S, N> {
        ShardedRwLockHandle {
            shards_handle: self.shards.buy_in(),
            parent: self,
        }
    }
}

unsafe impl<T: Sync, S: Strategy<ReaderShard<T>> + Sync, const N: usize> Sync
    for ShardedRwLock<T, S, N>
{
}
unsafe impl<T: Send, S: Strategy<ReaderShard<T>> + Send, const N: usize> Send
    for ShardedRwLock<T, S, N>
{
}

struct ShardedRwLockHandle<'a, T, S: Strategy<ReaderShard<T>>, const N: usize> {
    shards_handle: InlineBanditHandle<'a, ReaderShard<T>, RwLockStrategy<S>, N, 1>,
    parent: &'a ShardedRwLock<T, S, N>,
}

impl<'a, T, S: Strategy<ReaderShard<T>>, const N: usize> ShardedRwLockHandle<'a, T, S, N> {
    fn read(&mut self) -> Option<ReaderGuard<'a, '_, T>> {
        self.shards_handle
            .offer(LockInput {
                writer: &self.parent.writer,
                data: NonNull::from(&self.parent.item),
            })
            .ok()
    }

    fn write(&mut self) -> Option<WriteGuard<'a, T>> {
        self.shards_handle
            .poll(LockInput {
                writer: &self.parent.writer,
                data: NonNull::from(&self.parent.item),
            })
            .ok()
    }

    fn fork(&mut self) -> Self {
        Self {
            shards_handle: self.shards_handle.fork(),
            parent: self.parent,
        }
    }
}

fn main() {
    let lock = ShardedRwLock::<_, DCBO, 1>::new(42);

    let mut player1 = lock.new_root();
    let mut player2 = player1.fork();

    {
        let guard = player1.read().unwrap();
        assert_eq!(*guard, 42);
    }

    {
        let _reader = player1.read().unwrap();
        assert!(player2.write().is_none());
    }

    {
        let mut guard = player1.write().unwrap();
        *guard = 100;

        assert!(player2.read().is_none());
        assert!(player2.write().is_none());
    }

    assert_eq!(*player1.read().unwrap(), 100);

    let counter_lock = ShardedRwLock::<_, DCBO, 8>::new(0);
    let mut player0 = counter_lock.new_root();
    let threads = 8;
    let iterations_per_thread = 1000;

    thread::scope(|scope| {
        for _ in 0..threads {
            let mut thread_player = player0.fork();
            scope.spawn(move || {
                for _ in 0..iterations_per_thread {
                    loop {
                        if let Some(mut guard) = thread_player.write() {
                            *guard += 1;
                            break;
                        }
                        thread::yield_now();
                    }
                }
            });
        }
    });

    let final_value = *player0.read().unwrap();
    assert_eq!(final_value, threads * iterations_per_thread);
}
