use std::{
    future::{self, Future},
    ops::{Deref, DerefMut},
    sync::Arc, time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio::{
    select,
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, StreamExt};

use crate::{
    notify::{AsyncLockable, Notify, NotifyArc},
    MaybeSend,
};

pub trait Joinable<T> {
    fn join(&mut self) -> impl std::future::Future<Output = Result<T, Error>>;
}

pub trait IsRunning {
    fn running(&self) -> impl std::future::Future<Output = bool> + Send;
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

#[derive(Debug)]
pub enum TaskStatusError {
    BroadcastStreamRecvError(BroadcastStreamRecvError),
    Finished,
}

impl From<BroadcastStreamRecvError> for TaskStatusError {
    fn from(value: BroadcastStreamRecvError) -> Self {
        TaskStatusError::BroadcastStreamRecvError(value)
    }
}
#[allow(async_fn_in_trait)]
pub trait Task<S> {
    type AsyncLock: AsyncLockable<Status<S>>;

    fn status(&self) -> &Arc<Self::AsyncLock>;
    async fn running_status(&self)  -> Result<NotifyArc<Status<S>>, TaskStatusError>;
}

#[derive(Debug)]
pub enum Error {
    Pending,
    Aborted,
    Completed,
}

#[derive(Serialize, Deserialize, derive_more::Debug, PartialEq, Eq, Clone)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: Deserialize<'de>"))]
pub enum Status<S> {
    Pending,
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
                Status::Pending => Err(Error::Pending),
                Status::Running(state) => Ok(func(state)),
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

    pub fn pending(&self) -> bool {
        match self {
            Status::Pending => true,
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

impl<T, S> Default for AsyncTask<T, S> {
    fn default() -> Self {
        let (abort_tx, _) = mpsc::channel::<()>(1);
        let (_, output_rx) = mpsc::channel::<Result<T, Error>>(1);

        let status = Arc::new(Notify::new(Status::Pending));

        AsyncTask {
            abort_tx,
            output_rx,
            status,
            abort_on_drop: true,
        }
    }
}

impl<T, S: 'static > AsyncTask<T, S> {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        let status = Arc::get_mut(&mut self.status).unwrap();
        status.set_timeout(timeout);        
        self
    }
}

impl<T: MaybeSend + 'static, S: MaybeSend + Sync + 'static> AsyncTask<T, S> {
    pub fn spawn<F: FnOnce(&S) -> U, U: Future<Output = T> + MaybeSend + 'static>(
        &mut self,
        state: S,
        func: F,
    ) {
        let (abort_tx, mut abort_rx) = mpsc::channel::<()>(1);
        let (output_tx, output_rx) = mpsc::channel::<Result<T, Error>>(1);
        let future = func(&state);

        self.abort_tx = abort_tx;
        self.output_rx = output_rx;

        spawn_platform({
            let status = self.status.clone();
            async move {
                {
                    let mut lock = status.write().await;
                    *lock = Status::Running(state);
                    lock.notify();
                    if let Err(e) = lock.not_cloned(status.get_timeout()).await {
                        tracing::error!("Timeout waiting for writeable lock when starting task: {:?}", e);
                    }
                    
                };
                
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
                {*status.write().await = result};
            }
        });
    }
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
    
    async fn running_status(&self) -> Result<NotifyArc<Status<S>>, TaskStatusError> {
        let mut sub = self.status.subscribe().await;
        while let Some(next) = sub.next().await {
            let next = next?;
            if next.running() {
                return Ok(next);
            }
        }
        Err(TaskStatusError::Finished)
    }
}

impl<T: Send, S: Send + Sync + 'static> IsRunning for AsyncTask<T, S> {
    async fn running(&self) -> bool {
        match self.status.read().await.deref() {
            Status::Running(_) => true,
            _ => false,
        }
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
    let mut task: AsyncTask<T, S> = Default::default();
    task.spawn(state, func);
    task
}

#[cfg(not(target_family = "wasm"))]
fn spawn_platform<F: Future<Output = ()> + MaybeSend + 'static>(future: F) {
    tokio::task::spawn(future);
}

#[cfg(target_family = "wasm")]
fn spawn_platform<F: Future<Output = ()> + MaybeSend + 'static>(future: F) {
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_simple() {
        let mut task = spawn(10, |num| {
            let num = *num;
            async move {
                tokio::time::sleep(Duration::from_millis(num)).await;
                11
            }
        });
        assert_eq!(task.join().await.unwrap(), 11);
        assert_eq!(*task.status().read().await.deref(), Status::Completed);
        
        task.spawn(12, |num| {
            let num = *num;
            async move {
                tokio::time::sleep(Duration::from_millis(num)).await;
                13
            }
        });
        assert_eq!(task.join().await.unwrap(), 13);
        assert_eq!(*task.status().read().await.deref(), Status::Completed);
        drop(task);


    }
}
