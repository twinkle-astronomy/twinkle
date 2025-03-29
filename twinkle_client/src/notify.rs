use derive_more::{AsMut, AsRef, Deref, DerefMut, From};
use std::time::Duration;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use tokio::sync::RwLock;

use tokio_stream::StreamExt as _;

use crate::timeout;

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

#[derive(Deref, DerefMut, AsRef, AsMut, From, derive_more::Display, Debug)]
struct NoClone<T>(T);

impl<T> Clone for NoClone<T> {
    fn clone(&self) -> Self {
        panic!("This should never be cloned.  If it is cloned, something has gone wrong with twinkle_client::notify")
    }
}

pub struct ArcCounter<T> {
    value: std::sync::Arc<NoClone<T>>,
    count_tx: tokio::sync::watch::Sender<usize>,
    count_rx: tokio::sync::watch::Receiver<usize>,
}

// unsafe impl<T: Send> Send for ArcCounter<T> {}

impl<T> ArcCounter<T> {
    fn new(value: T) -> Self {
        let (count_tx, count_rx) = tokio::sync::watch::channel(1);
        Self {
            value: std::sync::Arc::new(NoClone(value)),
            count_tx, 
            count_rx,
        }
    }
}

impl<T: PartialEq> PartialEq for ArcCounter<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.ne(other)
    }
}

impl<T: Debug> Debug for ArcCounter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl<T> Deref for ArcCounter<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value.deref().deref()
    }
}

impl<T> DerefMut for ArcCounter<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        std::sync::Arc::make_mut(&mut self.value).deref_mut()
    }
}

impl<T> AsRef<T> for ArcCounter<T> {
    
    fn as_ref(&self) -> &T {
        self.value.as_ref()
    }
}

impl<T: std::fmt::Display> std::fmt::Display for ArcCounter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl<T> ArcCounter<T> {
    async fn not_cloned(&mut self) -> Result<(), tokio::sync::watch::error::RecvError> {
        loop {
            // Dumb / polling impl kept as a safety fallback
            // if std::sync::Arc::strong_count(&self.value) != 1 {
            //     sleep(Duration::from_millis(100)).await;
            // } else {
            //     break Ok(());
            // }
            {
                let count = self.count_rx.borrow();
                if *count == 1 {
                    return Ok(());
                }
            }
            self.count_rx.changed().await?;
        }
    }
}

impl<T> Clone for ArcCounter<T> {
    fn clone(&self) -> Self {
        let next_value = *self.count_tx.borrow() + 1;
        let _ = self.count_tx.send(next_value);
        Self {
            value: self.value.clone(),
            count_tx: self.count_tx.clone(),
            count_rx: self.count_rx.clone(),
        }
    }
}

impl<T> Drop for ArcCounter<T> {
    fn drop(&mut self) {
        let next_value = { *self.count_rx.borrow() - 1 };
        let _ = self.count_tx.send(next_value);
    }
}

pub async fn wait_fn<S, E, T: Clone + Send + 'static, F: FnMut(T) -> Result<Status<S>, E>>(
    mut stream: tokio_stream::wrappers::BroadcastStream<T>,
    dur: Duration,
    mut f: F,
) -> Result<S, Error<E>> {
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

/// The `Notify<T>` struct is a wrapper type that allows you to easily manage changes
/// to a value through internal mutability (it's a wrapper around a Mutex), and allows
/// other parts of your application to subscribe and wait for changes.
pub struct Notify<T> {
    subject: RwLock<NotifyArc<T>>,
    to_notify: tokio::sync::broadcast::Sender<NotifyArc<T>>,
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
            subject: RwLock::new(NotifyArc::new(value)),
            to_notify: tx,
        }
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
    /// Returns a [`NotifyMutexGuard<T>`](crate::twinkle_client::notify::NotifyMutexGuard) that allows you to read
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
    ///     assert_eq!(*notify.lock().await, 42);
    ///     {
    ///         let mut lock = notify.write().await;
    ///         *lock = 43;
    ///     }
    ///     assert_eq!(*notify.lock().await, 43);
    /// };
    /// ```
    pub async fn write(&self) -> NotifyMutexGuard<T> {
        let mut guard = self.subject.write().await;
        guard
            .not_cloned()
            .await
            .expect("Unable to await on guard not being cloned");
        NotifyMutexGuard {
            guard,
            to_notify: &self.to_notify,
            should_notify: false,
        }
    }
 
    #[deprecated(note = "please use `write` instead")]
    pub async fn lock(&self) -> NotifyMutexGuard<T> {
        let mut guard = self.subject.write().await;
        guard
            .not_cloned()
            .await
            .expect("Unable to await on guard not being cloned");
        NotifyMutexGuard {
            guard,
            to_notify: &self.to_notify,
            should_notify: false,
        }
    }

    /// Returns a [`NotifyMutexGuardRead<T>`](crate::twinkle_client::notify::NotifyMutexGuardRead) that allows you to read
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
    pub async fn read(&self) -> NotifyMutexGuardRead<T> {
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
    pub async fn subscribe(&self) -> tokio_stream::wrappers::BroadcastStream<NotifyArc<T>> {
        let subject = self.subject.read().await;
        let recv = self.to_notify.subscribe();
        self.to_notify.send(subject.deref().clone()).ok();
        tokio_stream::wrappers::BroadcastStream::new(recv)
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
    pub fn changes(&self) -> tokio_stream::wrappers::BroadcastStream<NotifyArc<T>> {
        tokio_stream::wrappers::BroadcastStream::new(self.to_notify.subscribe())
    }
}

pub struct NotifyMutexGuard<'a, T> {
    guard: tokio::sync::RwLockWriteGuard<'a, NotifyArc<T>>,
    to_notify: &'a tokio::sync::broadcast::Sender<NotifyArc<T>>,
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
        if self.should_notify {
            self.to_notify.send(self.guard.deref().clone()).ok();
        }
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
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::{thread, time::Duration};

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
                for _ in 0..10 {
                    let mut r = n.changes();

                    joins.push(tokio::spawn(async move {
                        let mut prev = r.next().await.unwrap().unwrap().deref().clone();
                        loop {
                            let j = r.next().await;
                            if let Some(Ok(j)) = j {
                                if *j == 90 {
                                    break;
                                }
                                assert_eq!(*j, prev + 1);
                                prev = *j;
                            } else {
                                break;
                            }
                        }
                    }));
                }
                for i in 0..=90 {
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
        let j = tokio::spawn(wait_fn::<(), (), _, _>(
            thread_notify.subscribe().await,
            Duration::from_secs(1),
            move |iteration| {
                {
                    let mut l = count_thread.lock().unwrap();
                    *l += 1;
                }
                if *iteration == 9 {
                    Ok(Status::Complete(()))
                } else {
                    Ok(Status::Pending)
                }
            },
        ));
        // ugly race-based thread syncronization
        thread::sleep(Duration::from_millis(100));
        for i in 0..=9 {
            let mut lock = notify.write().await;
            *lock = i;
        }

        j.await.unwrap().unwrap();
        {
            let lock = count.lock().unwrap();
            assert_eq!(*lock, 11);
        }
    }

    #[tokio::test]
    async fn test_cancel_wait_fn() {
        async {}.await;
        let notify: Arc<Notify<u32>> = Arc::new(Notify::new(0));
        let subscription = notify.subscribe().await;
        // let mut thread_subscription = subscription.clone();
        let fut = async move {
            wait_fn::<(), (), NotifyArc<u32>, _>(subscription, Duration::from_secs(10), |x| {
                // Will never be true
                if *x == 10 {
                    return Ok(Status::Complete(()));
                } else {
                    return Ok(Status::Pending);
                }
            })
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
}
