use futures_timer::Delay;
use std::future::{pending, Future};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

mod stream_ext;
pub use stream_ext::StreamExt;

pub mod agent;
pub mod notify;
pub mod task;

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
    /// use twinkle_client::OnDropFutureExt;
    /// use std::sync::{Mutex, Arc};
    /// async move {
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
    /// };
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

#[cfg(target_family = "wasm")]
pub trait MaybeSend {}
#[cfg(target_family = "wasm")]
impl<T> MaybeSend for T {}

#[cfg(target_family = "wasm")]
pub trait MaybeSync {}
#[cfg(target_family = "wasm")]
impl<T> MaybeSync for T {}

// Helper trait that requires Send for non-wasm
#[cfg(not(target_family = "wasm"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_family = "wasm"))]
impl<T: Send> MaybeSend for T {}

#[cfg(not(target_family = "wasm"))]
pub trait MaybeSync: Sync {}
#[cfg(not(target_family = "wasm"))]
impl<T: Sync> MaybeSync for T {}

#[derive(Debug)]
pub struct TimeoutError {}

pub async fn timeout<F: Future>(duration: Duration, future: F) -> Result<F::Output, TimeoutError> {
    tokio::select! {
        _ = Delay::new(duration) => Err(TimeoutError{}),
        result = future => Ok(result),
    }
}

pub async fn sleep(duration: Duration) {
    let _ = timeout(duration, pending::<()>()).await;
}


#[cfg(test)]
mod test {
    use std::time::Duration;

    #[tokio::test]
    async fn test_timeout() {

        assert!(super::timeout(Duration::from_millis(10), async move {
            loop {
                tokio::task::yield_now().await;
            }
        }).await.is_err())
    }
}