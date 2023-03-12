use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Mutex, MutexGuard},
};

use tokio_stream::StreamExt;

// https://stackoverflow.com/questions/74985153/implementing-drop-for-a-future-in-rust
pub trait OnDropFutureExt
where
    Self: Future + Sized,
{
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
    Abort(E),
}

impl<T> From<T> for Error<T> {
    fn from(value: T) -> Self {
        Error::Abort(value)
    }
}

impl<T> From<crossbeam_channel::SendError<T>> for Error<T> {
    fn from(_: crossbeam_channel::SendError<T>) -> Self {
        Error::Canceled
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
                    Ok(next) => f(next)?,
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
    pub fn new(value: T) -> Notify<T> {
        let (tx, _) = tokio::sync::broadcast::channel(1024);
        Notify {
            subject: Mutex::new(Arc::new(value)),
            to_notify: Mutex::new(tx),
        }
    }
}

pub struct NotifyMutexGuard<'a, T: Clone> {
    guard: MutexGuard<'a, Arc<T>>,
    to_notify: &'a Notify<T>,
    should_notify: bool,
}

impl<'a, T: Debug + Clone> Debug for NotifyMutexGuard<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_notify.subject.fmt(f)
    }
}

impl<'a, T: Clone> AsRef<T> for NotifyMutexGuard<'a, T> {
    fn as_ref(&self) -> &T {
        &self.guard
    }
}

impl<'a, T: Clone> Deref for NotifyMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a, T: Debug + Clone> DerefMut for NotifyMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.should_notify = true;
        Arc::make_mut(self.guard.deref_mut())
    }
}

impl<'a, T: Clone> Drop for NotifyMutexGuard<'a, T> {
    fn drop(&mut self) {
        if self.should_notify {
            let sender = self.to_notify.to_notify.lock().unwrap();
            sender.send(self.guard.deref().clone()).ok();
        }
        drop(&mut self.guard);
    }
}

impl<T: Clone + Debug + Sync + Send + 'static> Notify<T> {
    pub fn lock(&self) -> NotifyMutexGuard<T> {
        NotifyMutexGuard {
            guard: self.subject.lock().unwrap(),
            to_notify: self,
            should_notify: false,
        }
    }

    pub fn subscribe(&self) -> tokio_stream::wrappers::BroadcastStream<Arc<T>> {
        let subject = self.subject.lock().unwrap();
        let sender = self.to_notify.lock().unwrap();
        let recv = sender.subscribe();
        sender.send(subject.deref().clone()).ok();
        tokio_stream::wrappers::BroadcastStream::new(recv)
    }

    pub fn changes(&self) -> tokio_stream::wrappers::BroadcastStream<Arc<T>> {
        tokio_stream::wrappers::BroadcastStream::new(self.to_notify.lock().unwrap().subscribe())
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
                let mut l = n.lock();
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
                let mut with_mut = thread_n.lock();
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
            thread_notify.subscribe(),
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
            let mut lock = notify.lock();
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
        let subscription = notify.subscribe();
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
