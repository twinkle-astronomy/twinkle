use std::{
    future::{self, Future},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tokio::{
    select,
    sync::mpsc::{self, Receiver, Sender}
};

use crate::{notify::{AsyncLockable, Notify}, MaybeSend};

pub trait Joinable<T> {
    fn join(&mut self) -> impl std::future::Future<Output = Result<T, Error>>;
}

pub trait Abortable {
    fn abort(&self);
    fn abort_on_drop(mut self, abort: bool) -> Self
    where
        Self: Sized,
    {
        self.set_abort_on_drop(abort);
        self
    }

    fn set_abort_on_drop(&mut self, abort: bool);
}

pub trait Task<S> {
    type AsyncLock: AsyncLockable<Status<S>>;

    fn status(&self) -> &Arc<Self::AsyncLock>;
}

#[derive(Debug)]
pub enum Error {
    Aborted,
    Completed,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: Deserialize<'de>"))]
pub enum Status<S> {
    Running(S),
    Completed,
    Aborted,
}

impl<S> Status<S> {
        
    pub async fn with_state<'a, V, R, F>(&'a self, func: F) -> Result<V, Error>
    where
        F: FnOnce(&S) -> R + 'a,
        R: Future<Output = V> + 'a,
    {
        let future = {
            match self {
                Status::Running(status) => Ok(func(status)),
                Status::Completed => Err(Error::Completed),
                Status::Aborted => Err(Error::Aborted),
            }
        };
        match future {
            Ok(future) => Ok(future.await),
            Err(e) => Err(e),
        }
    }

    pub fn running(&self) -> bool {
        match self {
            Status::Running(_) => true,
            _ => false,
        }
    }
}
pub struct AsyncTask<T, S> {
    abort_tx: Sender<()>,
    output_rx: Receiver<Result<T, Error>>,
    status: Arc<Notify<Status<S>>>,
    abort_on_drop: bool,
}

impl<T, S> Drop for AsyncTask<T, S> {
    fn drop(&mut self) {
        if self.abort_on_drop {
            self.abort();
        }
    }
}
impl<A: Abortable, D: Deref<Target = A> + DerefMut> Abortable for D {
    fn abort(&self) {
        self.deref().abort()
    }

    fn set_abort_on_drop(&mut self, abort: bool) {
        self.deref_mut().set_abort_on_drop(abort);
    }
}
impl<T, S> Abortable for AsyncTask<T, S> {
    fn abort(&self) {
        let _ = self.abort_tx.try_send(());
    }

    fn set_abort_on_drop(&mut self, abort: bool) {
        self.abort_on_drop = abort;
    }
}
impl<T, S: Send + Sync + 'static> Task<S> for AsyncTask<T, S> {
    type AsyncLock = crate::notify::Notify<Status<S>>;

    fn status(&self) -> &Arc<Self::AsyncLock> {
        &self.status
    }
}

impl<T, S> Joinable<T> for AsyncTask<T, S> {
    async fn join(&mut self) -> Result<T, Error> {
        match self.output_rx.recv().await {
            Some(r) => r,
            None => Err(Error::Aborted),
        }
    }
}

pub fn spawn_with_state<
    S: MaybeSend + Sync + 'static,
    F: FnOnce(&S) -> U,
    U: Future<Output = ()> + MaybeSend + 'static,
>(
    state: S,
    func: F,
) -> AsyncTask<(), S> {
    spawn(state, func)
}

pub fn spawn_with_value<T: MaybeSend + 'static, U: Future<Output = T> + MaybeSend + 'static>(
    future: U,
) -> AsyncTask<T, ()> {
    spawn((), |_| future)
}

pub fn spawn<
    T: MaybeSend + 'static,
    S: MaybeSend + Sync + 'static,
    F: FnOnce(&S) -> U,
    U: Future<Output = T> + MaybeSend + 'static,
>(
    state: S,
    func: F,
) -> AsyncTask<T, S> {
    let (abort_tx, mut abort_rx) = mpsc::channel::<()>(1);
    let (output_tx, output_rx) = mpsc::channel::<Result<T, Error>>(1);
    let future = func(&state);
    let status = Arc::new(Notify::new(Status::Running(state)));

    spawn_platform({
        let status = status.clone();
        async move {
            let abort = async move {
                if let None = abort_rx.recv().await {
                    future::pending::<()>().await;
                }
            };

            let result = select! {
                r = future => {
                    if let Ok(_) = output_tx.try_send(Ok(r)) {
                        Status::Completed
                    } else {
                        Status::Aborted
                    }
                },
                _ = abort => {
                    if let Ok(_) =  output_tx.try_send(Err(Error::Aborted))  {
                        Status::Aborted
                    } else {
                        Status::Completed
                    }
                 },
            };
            *status.write().await = result;
        }
    });

    AsyncTask {
        abort_tx,
        output_rx,
        status,
        abort_on_drop: false,
    }
}

#[cfg(not(target_family = "wasm"))]
fn spawn_platform<F: Future<Output = ()> + MaybeSend + 'static>(future: F) {
    tokio::task::spawn(future);
}

#[cfg(target_family = "wasm")]
fn spawn_platform<F: Future<Output = ()> + MaybeSend + 'static>(future: F) {
    wasm_bindgen_futures::spawn_local(future);
}
