use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use tokio::sync::RwLock;

use tokio_stream::{Stream, StreamExt as _};

use crate::{timeout, TimeoutError};

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Timeout,
    Canceled,
    EndOfStream,
    Abort(E),
}

pub enum Status<S> {
    Pending,
    Complete(S),
}

pub struct ArcCounter<T> {
    value: Option<std::sync::Arc<T>>,
    clones: Arc<AtomicU64>,
    clone_changes: Arc<tokio::sync::Notify>,
}

impl<T> ArcCounter<T> {
    fn new(value: T) -> Self {
        let clones = Arc::new(AtomicU64::new(1));
        let clone_changes = Default::default();
        Self {
            value: Some(std::sync::Arc::new(value)),
            clones,
            clone_changes,
        }
    }
}

impl<T> From<T> for ArcCounter<T> {
    fn from(value: T) -> Self {
        ArcCounter::new(value)
    }
}

impl<T: PartialEq> PartialEq for ArcCounter<T> {
    fn eq(&self, other: &Self) -> bool {
        match &self.value {
            Some(value) => value.deref().eq(other.deref()),
            _ => false,
        }
    }
}

impl<T: Debug> Debug for ArcCounter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.value {
            Some(value) => value.fmt(f),
            None => panic!("ArcCounter used after drop"),
        }
    }
}

impl<T> Deref for ArcCounter<T> {
    type Target = T;
    fn deref(&self) -> &T {
        match &self.value {
            Some(value) => value.deref(),
            None => panic!("ArcCounter used after drop"),
        }
    }
}

impl<T> DerefMut for ArcCounter<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.value {
            Some(value) => std::sync::Arc::get_mut(value).unwrap(),
            None => panic!("ArcCounter used after drop"),
        }
    }
}

impl<T> AsRef<T> for ArcCounter<T> {
    fn as_ref(&self) -> &T {
        match &self.value {
            Some(value) => value.as_ref(),
            None => panic!("ArcCounter used after drop"),
        }
    }
}

impl<T: std::fmt::Display> std::fmt::Display for ArcCounter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.value {
            Some(value) => value.fmt(f),
            None => panic!("ArcCounter used after drop"),
        }
    }
}
impl<T: Clone> ArcCounter<T> {
    pub fn into_inner(self) -> T {
        self.deref().clone()
    }
}

impl<T> ArcCounter<T> {
    async fn not_cloned(&mut self, timeout: Duration) -> Result<(), TimeoutError> {
        crate::timeout(timeout, async {
            self.not_cloned_inner().await;
        })
        .await
    }

    async fn not_cloned_inner(&mut self) {
        // Polling based way of waiting for clone count to hit 0.
        loop {
            let count = Arc::strong_count(&self.value.as_ref().unwrap());
            if count == 1 {
                break;
            }
            if count > 2 {
                tracing::warn!("yielding for count: {}", count);
            }
            tokio::task::yield_now().await;
        }
        // Smarter way of waiting for clone count to hit 0.  Is racy.
        // loop {
        //     if self.clones.load(std::sync::atomic::Ordering::SeqCst) > 1 {
        //         self.clone_changes.notified().await;
        //     } else {
        //         break;
        //     }
        // }
    }
}

impl<T> Clone for ArcCounter<T> {
    fn clone(&self) -> Self {
        let value = self.value.clone();
        self.clones
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            value,
            clones: self.clones.clone(),
            clone_changes: self.clone_changes.clone(),
        }
    }
}

impl<T> Drop for ArcCounter<T> {
    fn drop(&mut self) {
        self.value = None;
        self.clones
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        self.clone_changes.notify_waiters();
    }
}

pub async fn wait_fn<
    P,
    E,
    T: Clone + Send,
    F: FnMut(T) -> Result<Status<P>, E>,
    S: Stream<Item = Result<T, BroadcastStreamRecvError>> + std::marker::Unpin,
>(
    stream: &mut S,
    dur: Duration,
    mut f: F,
) -> Result<P, Error<E>> {
    let res = timeout(dur, async {
        loop {
            if let Some(next) = stream.next().await {
                let status = match next {
                    Err(_) => return Err(Error::Canceled),
                    Ok(next) => match f(next) {
                        Ok(x) => x,
                        Err(e) => return Err(Error::Abort(e)),
                    },
                };

                if let Status::Complete(value) = status {
                    break Ok(value);
                }
            } else {
                return Err(Error::EndOfStream);
            }
        }
    })
    .await;

    match res {
        Ok(v) => return v,
        Err(_) => return Err(Error::Timeout),
    }
}

pub type NotifyArc<T> = ArcCounter<T>;

static NOTIFY_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The `Notify<T>` struct is a wrapper type that allows you to easily manage changes
/// to a value through internal mutability (it's a wrapper around a Mutex), and allows
/// other parts of your application to subscribe and wait for changes.
pub struct Notify<T> {
    id: usize,
    subject: RwLock<NotifyArc<T>>,
    to_notify: tokio::sync::broadcast::Sender<Option<NotifyArc<T>>>,
    timeout: Duration,
}

impl<T: Debug> Debug for Notify<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.subject.fmt(f)
    }
}

impl<T> Notify<T> {
    /// Returns a new `Notify<T>`
    /// # Example
    /// ```
    /// use twinkle_client::notify::Notify;
    /// let notify: Notify<i32> = Notify::new(42);
    /// ```
    pub fn new(value: T) -> Notify<T> {
        let (tx, _) = tokio::sync::broadcast::channel(1);
        Notify {
            id: NOTIFY_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            subject: RwLock::new(NotifyArc::new(value)),
            to_notify: tx,
            timeout: Duration::from_millis(1000),
        }
    }
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    pub fn get_timeout(&self) -> Duration {
        self.timeout
    }
}

impl<T> From<T> for Notify<T> {
    fn from(value: T) -> Self {
        Notify::new(value)
    }
}

impl<T: Default> Default for Notify<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: 'static> Notify<T> {
    /// Returns a [`NotifyMutexGuard<T>`](crate::notify::NotifyMutexGuard) that allows you to read
    /// (via the [Deref] trait) and write (via the [DerefMut] trait)
    /// the value stored in the `Notify<T>`.  The lock is exclusive,
    /// and only one lock will be held at a time. Use this method to find the current
    /// value, or to modify the value.  Will block until all changes sent to subscriptions
    /// have been consumed and dropped.
    ///
    /// # Example
    /// ```
    /// use twinkle_client::notify::Notify;
    /// async move {
    ///     let notify: Notify<i32> = Notify::new(42);
    ///     assert_eq!(*notify.write().await, 42);
    ///     {
    ///         let mut lock = notify.write().await;
    ///         *lock = 43;
    ///     }
    ///     assert_eq!(*notify.write().await, 43);
    /// };
    /// ```
    pub async fn write(&'_ self) -> NotifyMutexGuard<'_, T> {
        let mut notify_guard = NotifyMutexGuard {
            guard: self.subject.write().await,
            to_notify: &self.to_notify,
            should_notify: false,
        };
        if let Err(e) = notify_guard.not_cloned(self.timeout).await {
            tracing::error!("Timeout waiting for writeable lock: {:?}", e);
        }
        notify_guard
    }

    #[deprecated(note = "please use `write` instead")]
    pub async fn lock(&'_ self) -> NotifyMutexGuard<'_, T> {
        let mut notify_guard = NotifyMutexGuard {
            guard: self.subject.write().await,
            to_notify: &self.to_notify,
            should_notify: false,
        };
        if let Err(e) = notify_guard.not_cloned(self.timeout).await {
            tracing::error!("Timeout waiting for writeable lock: {:?}", e);
        }
        notify_guard
    }

    /// Returns a [`NotifyMutexGuardRead<T>`](crate::notify::NotifyMutexGuardRead) that allows you to read
    /// (via the [Deref] trait) the value stored in the `Notify<T>`.  The lock is read exclusive.  Any number of read locks
    /// may be held at once, but only one write lock.   Use this method to find the current value.
    ///
    /// # Example
    /// ```
    /// use twinkle_client::notify::Notify;
    /// async move {
    ///     let notify: Notify<i32> = Notify::new(42);
    ///     assert_eq!(*notify.read().await, 42);
    /// };
    /// ```
    pub async fn read(&'_ self) -> NotifyMutexGuardRead<'_, T> {
        NotifyMutexGuardRead {
            guard: self.subject.read().await,
        }
    }
}

impl<T: Send + Sync + 'static> Notify<T> {
    /// Returns a [`BroadcastStream<Arc<T>>`](tokio_stream::wrappers::BroadcastStream) of the values
    /// wrapped in an `Arc` held by `self` over time.  The returned stream's first value will be the current value
    /// at the time this method is called, and new values will be sent to the stream.  The stream will terminate
    /// when self is dropped. Calling this method locks the value momentarily to read the value, but the value is
    /// not locked on return.
    ///
    /// # Example
    /// ```
    /// use twinkle_client::notify::Notify;
    /// use tokio_stream::StreamExt;
    /// use std::sync::Arc;
    /// use std::ops::Deref;
    /// async fn increment( notify: &mut Notify<i32>) {
    ///     let mut lock = notify.write().await;
    ///     *lock = *lock + 1;
    /// }
    ///
    /// async move {
    ///     let mut sub = {
    ///         let mut notify = Notify::new(0);
    ///         let sub = notify.subscribe().await;
    ///         increment(&mut notify).await;
    ///         increment(&mut notify).await;
    ///         increment(&mut notify).await;
    ///         sub
    ///     };
    ///     
    ///     assert_eq!(sub.next().await.unwrap().unwrap().deref(), &0);
    ///     assert_eq!(sub.next().await.unwrap().unwrap().deref(), &1);
    ///     assert_eq!(sub.next().await.unwrap().unwrap().deref(), &2);
    ///     assert_eq!(sub.next().await.unwrap().unwrap().deref(), &3);
    ///     assert_eq!(sub.next().await, None);
    /// };
    /// ```
    pub async fn subscribe(
        &self,
    ) -> impl Stream<Item = Result<NotifyArc<T>, BroadcastStreamRecvError>> {
        let subject = self.subject.read().await;
        let recv = self.to_notify.subscribe();
        let current_value = subject.deref().clone();

        // Create a stream that starts with the current value then continues with broadcast values
        let s = tokio_stream::once(Ok(Some(current_value)))
            .chain(tokio_stream::wrappers::BroadcastStream::new(recv))
            .filter_map(|f| match f {
                Ok(Some(item)) => Some(Ok(item)),
                Ok(None) => None,
                Err(e) => Some(Err(e)),
            });
        s
    }

    /// Returns a [`BroadcastStream<Arc<T>>`](tokio_stream::wrappers::BroadcastStream) of the values
    /// wrapped in an `Arc` held by `self` over time.  Unlike `subscribe`, only new values will be sent to the
    /// stream.  The stream will terminate when self is dropped.
    ///
    /// # Example
    /// ```
    /// use twinkle_client::notify::Notify;
    /// use tokio_stream::StreamExt;
    /// use std::sync::Arc;
    /// use std::ops::Deref;
    /// async fn increment( notify: &mut Notify<i32>) {
    ///     let mut lock = notify.write().await;
    ///     *lock = *lock + 1;
    /// }
    ///
    /// async move {
    ///     let mut sub = {
    ///         let mut notify = Notify::new(0);
    ///         let sub = notify.changes();
    ///         increment(&mut notify).await;
    ///         increment(&mut notify).await;
    ///         increment(&mut notify).await;
    ///         sub
    ///     };
    ///     
    ///     assert_eq!(sub.next().await.unwrap().unwrap().deref(), &1);
    ///     assert_eq!(sub.next().await.unwrap().unwrap().deref(), &2);
    ///     assert_eq!(sub.next().await.unwrap().unwrap().deref(), &3);
    ///     assert_eq!(sub.next().await, None);
    /// };
    /// ```
    pub fn changes(&self) -> impl Stream<Item = Result<NotifyArc<T>, BroadcastStreamRecvError>> {
        tokio_stream::wrappers::BroadcastStream::new(self.to_notify.subscribe()).filter_map(|f| {
            match f {
                Ok(Some(item)) => Some(Ok(item)),
                Ok(None) => None,
                Err(e) => Some(Err(e)),
            }
        })
    }
}

pub struct NotifyMutexGuard<'a, T> {
    guard: tokio::sync::RwLockWriteGuard<'a, NotifyArc<T>>,
    to_notify: &'a tokio::sync::broadcast::Sender<Option<NotifyArc<T>>>,
    should_notify: bool,
}

impl<'a, T: Debug> Debug for NotifyMutexGuard<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.guard.fmt(f)
    }
}

impl<'a, T> AsRef<T> for NotifyMutexGuard<'a, T> {
    fn as_ref(&self) -> &T {
        &self.guard
    }
}

impl<'a, T> Deref for NotifyMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a, T> DerefMut for NotifyMutexGuard<'a, T> {
    /// Mutably dereferences the value.  If the value is currently holding
    /// a previous value then the wrapped value T will be cloned.
    /// See [`Arc::make_mut`](std::sync::Arc::make_mut) for more details.
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.should_notify = true;
        self.guard.deref_mut()
    }
}

impl<'a, T> Drop for NotifyMutexGuard<'a, T> {
    /// Executes the destructor for this type. [Read more](core::ops::Drop::drop).
    /// If this lock has created a mutable reference
    /// then the current value will be broadcast to all broadcast streams listening for changes.
    fn drop(&mut self) {
        self.notify();
    }
}

impl<'a, T> NotifyMutexGuard<'a, T> {
    pub fn notify(&mut self) {
        if self.should_notify {
            self.to_notify.send(Some(self.guard.deref().clone())).ok();
            self.should_notify = false;
        }
    }

    pub async fn not_cloned(&mut self, timeout: Duration) -> Result<(), TimeoutError> {
        let mut ret: Result<(), TimeoutError> = Ok(());
        loop {
            match self.guard.not_cloned(timeout).await {
                Ok(_) => break,
                Err(e) => {
                    ret = Err(e);
                    let _ = {
                        tracing::error!("Timeout waitng for not_cloned, sending None");
                        self.to_notify.send(None)
                    };
                }
            }
        }
        ret
    }
}

pub struct NotifyMutexGuardRead<'a, T> {
    guard: tokio::sync::RwLockReadGuard<'a, NotifyArc<T>>,
}

impl<'a, T: Debug> Debug for NotifyMutexGuardRead<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.guard.fmt(f)
    }
}

impl<'a, T> AsRef<T> for NotifyMutexGuardRead<'a, T> {
    fn as_ref(&self) -> &T {
        &self.guard
    }
}

impl<'a, T> Deref for NotifyMutexGuardRead<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

pub trait AsyncLockable<T> {
    type Lock<'a>: Deref<Target = T> + DerefMut + 'a
    where
        Self: 'a;

    type WriteLock<'a>: Deref<Target = T> + DerefMut + 'a
    where
        Self: 'a;
    type ReadLock<'a>: Deref<Target = T> + 'a
    where
        Self: 'a;

    fn new(value: T) -> Self;

    fn lock(&self) -> impl std::future::Future<Output = Self::Lock<'_>>;
    fn write(&self) -> impl std::future::Future<Output = Self::WriteLock<'_>>;
    fn read(&self) -> impl std::future::Future<Output = Self::ReadLock<'_>>;
}

impl<T: Send + 'static> AsyncLockable<T> for Notify<T> {
    type Lock<'a> = NotifyMutexGuard<'a, T>;
    type WriteLock<'a> = NotifyMutexGuard<'a, T>;
    type ReadLock<'a> = NotifyMutexGuardRead<'a, T>;

    fn new(value: T) -> Self {
        Notify::new(value)
    }

    async fn lock(&self) -> Self::Lock<'_> {
        Notify::write(self).await
    }

    async fn write(&self) -> Self::WriteLock<'_> {
        Notify::write(self).await
    }

    async fn read(&self) -> Self::ReadLock<'_> {
        Notify::read(self).await
    }
}

impl<T: 'static> AsyncLockable<T> for tokio::sync::Mutex<T> {
    type Lock<'a> = tokio::sync::MutexGuard<'a, T>;
    type WriteLock<'a> = tokio::sync::MutexGuard<'a, T>;
    type ReadLock<'a> = tokio::sync::MutexGuard<'a, T>;

    fn new(value: T) -> Self {
        Self::new(value)
    }

    async fn lock(&self) -> Self::Lock<'_> {
        self.lock().await
    }

    async fn write(&self) -> Self::WriteLock<'_> {
        self.lock().await
    }

    async fn read(&self) -> Self::ReadLock<'_> {
        self.lock().await
    }
}

#[cfg(test)]
mod test {
    use tracing_test::traced_test;

    use crate::{sleep, task};
    // use crate::task::spawn_with_value;

    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;

    #[derive(
        derive_more::Deref,
        derive_more::DerefMut,
        derive_more::AsRef,
        derive_more::AsMut,
        derive_more::From,
        derive_more::Display,
        Debug,
    )]
    struct NoClone<T>(T);

    impl<T> Clone for NoClone<T> {
        fn clone(&self) -> Self {
            panic!("This should never be cloned.  If it is cloned, something has gone wrong with twinkle_client::notify")
        }
    }
    #[tokio::test]
    async fn test_no_clone() {
        tokio::time::timeout(Duration::from_secs(1), async move {
            let (modify_task, sub_task) = {
                let notify = Notify::new(NoClone(0));

                let mut sub = notify.subscribe().await;

                (
                    tokio::spawn(async move {
                        {
                            *notify.write().await = NoClone(1);
                        }
                        {
                            *notify.write().await = NoClone(2);
                        }
                        {
                            *notify.write().await = NoClone(3);
                        }
                        {
                            *notify.write().await = NoClone(4);
                        }
                    }),
                    tokio::spawn(async move {
                        loop {
                            match sub.next().await {
                                Some(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                                _ => break,
                            }
                        }
                    }),
                )
            };

            sub_task.await.unwrap();
            modify_task.await.unwrap();
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_sequence() {
        tokio::time::timeout(Duration::from_secs(1), async move {
            let mut joins = Vec::new();
            {
                let n = Arc::new(Notify::new(-1));
                for _ in 0..3 {
                    let mut r = n.subscribe().await;

                    joins.push(tokio::spawn(async move {
                        let mut seen: Vec<i32> = Default::default();

                        loop {
                            tokio::time::sleep(Duration::from_micros(10)).await;
                            let j = r.next().await;
                            match j {
                                Some(Ok(j)) => {
                                    seen.push(*j);
                                }
                                Some(Err(e)) => {
                                    dbg!(e);
                                }
                                _ => break,
                            }
                        }
                        dbg!(seen);
                    }));
                }
                for i in 0..=9 {
                    dbg!(i);
                    let mut l = n.write().await;
                    *l = i;
                }
            }
            for x in joins {
                x.await.unwrap();
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_notify_on_mut() {
        let n = Arc::new(Notify::new(0));
        let mut r = n.changes();
        let thread_n = n.clone();
        let j = tokio::spawn(async move {
            {
                let _no_mut = thread_n.write().await;
            }
            {
                let mut with_mut = thread_n.write().await;
                *with_mut = 1;
            }
        });

        let update = r.next().await.unwrap().expect("stream");

        assert!(tokio::time::timeout(Duration::from_millis(100), r.next())
            .await
            .is_err());
        assert_eq!(*update, 1);

        j.await.unwrap();
    }

    #[tokio::test]
    async fn test_wakes() {
        let notify: Arc<Notify<u32>> = Arc::new(Notify::new(0));

        let count = Arc::new(StdMutex::new(0));
        let count_thread = count.clone();
        let thread_notify = notify.clone();
        let mut sub = thread_notify.subscribe().await;
        let task = tokio::spawn(async move {
            wait_fn::<(), (), _, _, _>(&mut sub, Duration::from_secs(1), move |iteration| {
                dbg!(iteration.deref());
                {
                    let mut l = count_thread.lock().unwrap();
                    *l += 1;
                }
                if *iteration == 9 {
                    Ok(Status::Complete(()))
                } else {
                    Ok(Status::Pending)
                }
            })
            .await
        });
        for i in 0..=9 {
            let mut lock = notify.write().await;
            *lock = i;
        }

        task.await.unwrap().unwrap();
        {
            let lock = count.lock().unwrap();
            assert_eq!(*lock, 11);
        }
    }

    #[tokio::test]
    async fn test_cancel_wait_fn() {
        async {}.await;
        let notify: Arc<Notify<u32>> = Arc::new(Notify::new(0));
        let mut subscription = notify.subscribe().await;
        // let mut thread_subscription = subscription.clone();
        let fut = async move {
            wait_fn::<(), (), NotifyArc<u32>, _, _>(
                &mut subscription,
                Duration::from_secs(10),
                |x| {
                    // Will never be true
                    if *x == 10 {
                        return Ok(Status::Complete(()));
                    } else {
                        return Ok(Status::Pending);
                    }
                },
            )
            .await
        };
        let j = tokio::spawn(fut);

        j.abort();
        assert!(j.await.is_err());
    }

    #[tokio::test]
    async fn test_nested_notifies() {
        tokio::time::timeout(Duration::from_secs(1), async move {
            let (do_changes, outer_sub_future, inner_sub_future) = {
                let nested: Arc<Notify<Arc<(Notify<i32>, Notify<i32>)>>> = Default::default();

                let (mut inner_sub, mut outer_sub) = {
                    let inner_sub = nested.read().await.1.subscribe().await;
                    let outer_sub = nested.subscribe().await;

                    (inner_sub, outer_sub)
                };

                (
                    async move {
                        *nested.read().await.1.write().await += 1;
                        *nested.read().await.0.write().await += 1;

                        *nested.read().await.1.write().await += 1;
                    },
                    async move {
                        loop {
                            match outer_sub.next().await {
                                Some(Ok(m)) => {
                                    dbg!(m);
                                }
                                _ => break,
                            }
                        }
                    },
                    async move {
                        let mut expected = vec![0, 1, 2].into_iter();
                        loop {
                            let i = match expected.next() {
                                Some(i) => i,
                                None => break,
                            };
                            match inner_sub.next().await {
                                Some(Ok(a)) => {
                                    dbg!(i, *a);
                                    assert_eq!(i, *a)
                                }
                                _ => panic!("Not enough entries"),
                            }
                        }
                    },
                )
            };
            tokio::join!(do_changes, inner_sub_future, outer_sub_future);
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    #[traced_test]
    async fn test_with_notify_timeout() {
        let mut notify = Notify::new(0);
        notify.set_timeout(Duration::MAX);
        let mut sub = notify.subscribe().await;
        let mut task = task::AsyncTask::default();
        task.spawn((), |_| async move {
            for i in 0..1000 {
                tracing::info!("Updating to {}", i);
                {
                    let mut lock = timeout(Duration::from_millis(11), notify.write())
                        .await
                        .unwrap();
                    *lock = i;
                    drop(lock);
                }
                sleep(Duration::from_millis(20)).await;
            }
        });

        let mut counter = 0;
        while let Some(item) = sub.next().await {
            tracing::info!("Processing item: {:?}", item);
            counter += 1;
            tracing::info!("Waiting: {}ms", counter);
            sleep(Duration::from_millis(counter)).await;
        }
    }
}
