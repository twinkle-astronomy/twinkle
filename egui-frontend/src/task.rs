use std::{
    future::{self, Future},
    sync::Arc,
};

use tokio::{
    select,
    sync::{mpsc::{self, Receiver, Sender}, Mutex},
};

pub trait Joinable<T> {
    fn join(&mut self) -> impl std::future::Future<Output = Result<T, Error>>;
}

pub trait Task<S> {
    fn abort(&self);
    fn status(&self) -> &Arc<Mutex<Status<S>>>;

    fn running(&self) -> impl Future<Output = bool>   {
        async {
            let status = self.status().lock().await;
            match *status {
                Status::Running(_) => true,
                _ => false,
            }    
        }
    }
}

pub enum Error {
    Aborted,
}

pub enum Status<S> {
    Running(S),
    Completed,
    Aborted,
}

pub struct AsyncTask<T, S> {
    abort_tx: Sender<()>,
    output_rx: Receiver<Result<T, Error>>,
    status: Arc<Mutex<Status<S>>>,
    abort_on_drop: bool,
}

impl<T, S> AsyncTask<T, S> {
    pub fn abort_on_drop(mut self, abort: bool) -> Self {
        self.abort_on_drop = abort;
        self
    }
}

impl<T, S> Drop for AsyncTask<T, S> {
    fn drop(&mut self) {
        if self.abort_on_drop {
            self.abort();
        }
    }
}
impl<T, S> Task<S> for AsyncTask<T, S> {
    fn abort(&self) {
        let _ = self.abort_tx.try_send(());
    }

    fn status(&self) -> &Arc<Mutex<Status<S>>> {
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
    S: 'static,
    F: FnOnce(&S) -> U,
    U: Future<Output = ()> + 'static,
>(
    state: S,
    func: F,
) -> AsyncTask<(), S> {
    spawn(state, func)
}

pub fn spawn_with_value<
    T: 'static,
    U: Future<Output = T> + 'static,
>(
    future: U,
) -> AsyncTask<T, ()> {
    spawn((), |_| future)
}

pub fn spawn<
    T: 'static,
    S: 'static,
    F: FnOnce(&S) -> U,
    U: Future<Output = T> + 'static,
>(
    state: S,
    func: F,
) -> AsyncTask<T, S> {
    let (abort_tx, mut abort_rx) = mpsc::channel::<()>(1);
    let (output_tx, output_rx) = mpsc::channel::<Result<T, Error>>(1);
    let future = func(&state);
    let status = Arc::new(Mutex::new(Status::Running(state)));
    

    wasm_bindgen_futures::spawn_local({
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
            *status.lock().await = result;
        }
    });

    AsyncTask {
        abort_tx,
        output_rx,
        status,
        abort_on_drop: false,
    }
}
