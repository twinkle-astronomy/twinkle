use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, PoisonError};
use std::task::{Context, Poll};
use std::time::Duration;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Mutex, MutexGuard},
};

use tokio_stream::StreamExt;

// https://stackoverflow.com/questions/74985153/implementing-drop-for-a-future-in-rust

/// Trait allowing you to attach a function to a [Future] that will be called when
/// the future is dropped.  
pub trait OnDropFutureExt
where
    Self: Future + Sized,
{
    /// Wraps the future with an OnDropFuture that will execute the given function
    /// when the future is dropped.  This is useful for situations where some resources need
    /// to be cleaned up when the future goes away.  Note; the function registered with this
    /// method will *always* run when the future is dropped which happens when a future is run
    /// to completion, and when it isn't.
    /// # Example
    /// ```
    /// use indi::client::notify::OnDropFutureExt;
    /// use std::sync::{Mutex, Arc};
    /// #[tokio::main]
    /// async fn main() {
    ///     let val1 = Arc::new(Mutex::new(0));
    ///     let val2 = val1.clone();
    ///     let val3 = val1.clone();
    ///     let future = async {
    ///         println!("In the future!");
    ///         let mut val_lock = val1.lock().unwrap();
    ///         assert_eq!(*val_lock, 0);
    ///         *val_lock += 1;
    ///     }.on_drop(move ||  {
    ///         println!("On the drop");
    ///         let mut val_lock = val2.lock().unwrap();
    ///         assert_eq!(*val_lock, 1);
    ///         *val_lock += 1;
    ///     });
    ///     future.await;
    ///     assert_eq!(*val3.lock().unwrap(), 2);
    /// }
    fn on_drop<D: FnMut()>(self, on_drop: D) -> OnDropFuture<Self, D>;
}
impl<F: Future> OnDropFutureExt for F {
    fn on_drop<D: FnMut()>(self, on_drop: D) -> OnDropFuture<Self, D> {
        OnDropFuture {
            inner: self,
            on_drop,
        }
    }
}

pub struct OnDropFuture<F: Future, D: FnMut()> {
    inner: F,
    on_drop: D,
}
impl<F: Future, D: FnMut()> OnDropFuture<F, D> {
    // See: https://doc.rust-lang.org/std/pin/#pinning-is-structural-for-field
    fn get_mut_inner(self: Pin<&mut Self>) -> Pin<&mut F> {
        unsafe { self.map_unchecked_mut(|s| &mut s.inner) }
    }

    // See: https://doc.rust-lang.org/std/pin/#pinning-is-not-structural-for-field
    fn get_mut_on_drop(self: Pin<&mut Self>) -> &mut D {
        unsafe { &mut self.get_unchecked_mut().on_drop }
    }
}
impl<F: Future, D: FnMut()> Future for OnDropFuture<F, D> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        self.get_mut_inner().poll(cx)
    }
}
impl<F: Future, D: FnMut()> Drop for OnDropFuture<F, D> {
    fn drop(&mut self) {
        // See: https://doc.rust-lang.org/std/pin/#drop-implementation
        inner_drop(unsafe { Pin::new_unchecked(self) });
        fn inner_drop<F: Future, D: FnMut()>(this: Pin<&mut OnDropFuture<F, D>>) {
            this.get_mut_on_drop()();
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Timeout,
    Canceled,
    EndOfStream,
    PoisonError,
    Abort(E),
}

impl<T> From<crossbeam_channel::SendError<T>> for Error<T> {
    fn from(_: crossbeam_channel::SendError<T>) -> Self {
        Error::Canceled
    }
}

impl<E, T> From<PoisonError<E>> for Error<T> {
    fn from(_: PoisonError<E>) -> Self {
        Error::PoisonError
    }
}
pub enum Status<S> {
    Pending,
    Complete(S),
}

pub async fn wait_fn<S, E, T: Clone + Send + 'static, F: FnMut(T) -> Result<Status<S>, E>>(
    mut stream: tokio_stream::wrappers::BroadcastStream<T>,
    dur: Duration,
    mut f: F,
) -> Result<S, Error<E>> {
    let res = tokio::time::timeout(dur, async {
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

/// The `Notify<T>` struct is a wrapper type that allows you to easily manage changes
/// to a value through internal mutability (it's a wrapper around a Mutex), and allows
/// other parts of your application to subscribe and wait for changes.
pub struct Notify<T> {
    subject: Mutex<Arc<T>>,
    to_notify: Mutex<tokio::sync::broadcast::Sender<Arc<T>>>,
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
    /// use indi::client::notify::Notify;
    /// let notify: Notify<i32> = Notify::new(42);
    /// ```
    pub fn new(value: T) -> Notify<T> {
        let (tx, _) = tokio::sync::broadcast::channel(1024);
        Notify {
            subject: Mutex::new(Arc::new(value)),
            to_notify: Mutex::new(tx),
        }
    }
}

impl<T: Debug + Sync + Send + 'static> Notify<T> {
    /// Returns a [`NotifyMutexGuard<T>`](crate::client::notify::NotifyMutexGuard) that allows you to read
    /// (via the [Deref] trait) and write (via the [DerefMut] trait)
    /// the value stored in the `Notify<T>`.  The lock is exclusive,
    /// and only one lock will be held at a time. Use this method to find the current
    /// value, or to modify the value.
    /// # Errors
    /// If another user of this notify panicked while holding the lock, then this call will return an error.  See [std::sync::Mutex] for more details.
    ///
    /// # Example
    /// ```
    /// use indi::client::notify::Notify;
    /// let notify: Notify<i32> = Notify::new(42);
    /// assert_eq!(*notify.lock().unwrap(), 42);
    /// {
    ///     let mut lock = notify.lock().unwrap();
    ///     *lock = 43;
    /// }
    /// assert_eq!(*notify.lock().unwrap(), 43);
    /// ```
    pub fn lock(&self) -> Result<NotifyMutexGuard<T>, PoisonError<MutexGuard<Arc<T>>>> {
        Ok(NotifyMutexGuard {
            guard: self.subject.lock()?,
            to_notify: self,
            should_notify: false,
        })
    }

    /// Returns a [`BroadcastStream<Arc<T>>`](tokio_stream::wrappers::BroadcastStream) of the values
    /// wrapped in an `Arc` held by `self` over time.  The returned stream's first value will be the current value
    /// at the time this method is called, and new values will be sent to the stream.  The stream will terminate
    /// when self is dropped. Calling this method locks the value momentarily to read the value, but the value is
    /// not locked on return.  
    /// # Errors
    /// If another user of this notify panicked while holding the lock, then this call will return an error.  See [std::sync::Mutex] for more details.
    ///
    /// # Example
    /// ```
    /// use indi::client::notify::Notify;
    /// use tokio_stream::StreamExt;
    /// use std::sync::Arc;
    /// fn increment( notify: &mut Notify<i32>) {
    ///     let mut lock = notify.lock().unwrap();
    ///     *lock = *lock + 1;
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut sub = {
    ///         let mut notify = Notify::new(0);
    ///         let sub = notify.subscribe().unwrap();
    ///         increment(&mut notify);
    ///         increment(&mut notify);
    ///         increment(&mut notify);
    ///         sub
    ///     };
    ///     
    ///     assert_eq!(sub.next().await.unwrap().unwrap(), Arc::new(0));
    ///     assert_eq!(sub.next().await.unwrap().unwrap(), Arc::new(1));
    ///     assert_eq!(sub.next().await.unwrap().unwrap(), Arc::new(2));
    ///     assert_eq!(sub.next().await.unwrap().unwrap(), Arc::new(3));
    ///     assert_eq!(sub.next().await, None);
    /// }
    /// ```
    pub fn subscribe(
        &self,
    ) -> Result<tokio_stream::wrappers::BroadcastStream<Arc<T>>, PoisonError<MutexGuard<Arc<T>>>>
    {
        let subject = self.subject.lock()?;
        let sender = self.to_notify.lock().unwrap();
        let recv = sender.subscribe();
        sender.send(subject.deref().clone()).ok();
        Ok(tokio_stream::wrappers::BroadcastStream::new(recv))
    }

    /// Returns a [`BroadcastStream<Arc<T>>`](tokio_stream::wrappers::BroadcastStream) of the values
    /// wrapped in an `Arc` held by `self` over time.  Unlike `subscribe`, only new values will be sent to the
    /// stream.  The stream will terminate when self is dropped.
    ///
    /// # Example
    /// ```
    /// use indi::client::notify::Notify;
    /// use tokio_stream::StreamExt;
    /// use std::sync::Arc;
    /// fn increment( notify: &mut Notify<i32>) {
    ///     let mut lock = notify.lock().unwrap();
    ///     *lock = *lock + 1;
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut sub = {
    ///         let mut notify = Notify::new(0);
    ///         let sub = notify.changes();
    ///         increment(&mut notify);
    ///         increment(&mut notify);
    ///         increment(&mut notify);
    ///         sub
    ///     };
    ///     
    ///     assert_eq!(sub.next().await.unwrap().unwrap(), Arc::new(1));
    ///     assert_eq!(sub.next().await.unwrap().unwrap(), Arc::new(2));
    ///     assert_eq!(sub.next().await.unwrap().unwrap(), Arc::new(3));
    ///     assert_eq!(sub.next().await, None);
    /// }
    /// ```
    pub fn changes(&self) -> tokio_stream::wrappers::BroadcastStream<Arc<T>> {
        tokio_stream::wrappers::BroadcastStream::new(self.to_notify.lock().unwrap().subscribe())
    }
}

pub struct NotifyMutexGuard<'a, T> {
    guard: MutexGuard<'a, Arc<T>>,
    to_notify: &'a Notify<T>,
    should_notify: bool,
}

impl<'a, T: Debug> Debug for NotifyMutexGuard<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_notify.subject.fmt(f)
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

impl<'a, T: Clone> DerefMut for NotifyMutexGuard<'a, T> {
    /// Mutably dereferences the value.  If the value is currently holding
    /// a previous value then the wrapped value T will be cloned.
    /// See [`Arc::make_mut`](std::sync::Arc::make_mut) for more details.
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.should_notify = true;
        Arc::make_mut(self.guard.deref_mut())
    }
}

impl<'a, T> Drop for NotifyMutexGuard<'a, T> {
    /// Executes the destructor for this type. [Read more](core::ops::Drop::drop).
    /// If this lock has created a mutable reference
    /// then the current value will be broadcast to all broadcast streams listening for changes.
    fn drop(&mut self) {
        if self.should_notify {
            let sender = self.to_notify.to_notify.lock().unwrap();
            sender.send(self.guard.deref().clone()).ok();
        }
        drop(&mut self.guard);
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use std::{thread, time::Duration};

    #[tokio::test]
    async fn test_sequence() {
        let mut joins = Vec::new();
        {
            let n = Arc::new(Notify::new(-1));

            for _ in 0..10 {
                let mut r = n.changes();
                joins.push(tokio::spawn(async move {
                    let mut prev = r.next().await.unwrap().unwrap();
                    loop {
                        let j = r.next().await;
                        if let Some(Ok(j)) = j {
                            if *j == 90 {
                                break;
                            }
                            assert_eq!(*j, *prev + 1);
                            prev = j;
                        } else {
                            break;
                        }
                    }
                }));
            }

            for i in 0..=90 {
                let mut l = n.lock().unwrap();
                *l = i;
            }
        }
        for x in joins {
            x.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_notify_on_mut() {
        let n = Arc::new(Notify::new(0));
        let mut r = n.changes();
        let thread_n = n.clone();
        let j = thread::spawn(move || {
            {
                let _no_mut = thread_n.lock();
            }
            {
                let mut with_mut = thread_n.lock().unwrap();
                *with_mut = 1;
            }
        });

        let update = r.next().await.unwrap().expect("stream");

        assert!(tokio::time::timeout(Duration::from_millis(100), r.next())
            .await
            .is_err());
        assert_eq!(*update, 1);

        j.join().unwrap();
    }

    #[tokio::test]
    async fn test_wakes() {
        let notify: Arc<Notify<u32>> = Arc::new(Notify::new(0));

        let count = Arc::new(Mutex::new(0));
        let count_thread = count.clone();
        let thread_notify = notify.clone();
        let j = tokio::spawn(wait_fn::<(), (), _, _>(
            thread_notify.subscribe().unwrap(),
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
            let mut lock = notify.lock().unwrap();
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
        let subscription = notify.subscribe().unwrap();
        // let mut thread_subscription = subscription.clone();
        let fut = async move {
            wait_fn::<(), (), Arc<u32>, _>(subscription, Duration::from_secs(10), |x| {
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
}
