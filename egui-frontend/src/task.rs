use std::{future::{self, Future}, rc::Rc, sync::atomic::{AtomicBool, Ordering}};

use tokio::{
    select,
    sync::mpsc::{self, Receiver, Sender},
};

pub trait Joinable<T> {
    fn join(&mut self) -> impl std::future::Future<Output = Result<T, Error>>;

}
pub trait Task {
    fn abort(&self);
    fn status(&self) -> Status;
}
pub enum Error {
    Aborted,
}

#[derive(PartialEq)]
pub enum Status {
    Running,
    Completed,
    Aborted,
}

pub struct AsyncTask<T> {
    abort_tx: Sender<()>,
    output_rx: Receiver<Result<T, Error>>,
    was_aborted: Rc<AtomicBool>,
}
impl<T> Joinable<T> for AsyncTask<T> {

    async fn join(&mut self) -> Result<T, Error> {
        match self.output_rx.recv().await {
            Some(r) => r,
            None => Err(Error::Aborted),
        }
    }

}

impl<T> Task for AsyncTask<T> {
    fn abort(&self) {
        let _ = self.abort_tx.try_send(());
    }

    fn status(&self) -> Status {
        match self.output_rx.is_closed() {
            false => Status::Running,
            true => {
                match self.was_aborted.load(Ordering::SeqCst) {
                    true => Status::Aborted,
                    false => Status::Completed,
                }
            },
        }
    } 
}

pub fn spawn<T: 'static, U: Future<Output = T> + 'static>(future: U) -> AsyncTask<T> {
    let (abort_tx, mut abort_rx) = mpsc::channel::<()>(1);
    let (output_tx, output_rx) = mpsc::channel::<Result<T, Error>>(1);
    let was_aborted = Rc::new(AtomicBool::new(false));

    wasm_bindgen_futures::spawn_local({        
        let was_aborted = was_aborted.clone();
            async move {
            let abort = async move {
                if let None = abort_rx.recv().await {
                    future::pending::<()>().await;
                }
            };

            select! {
                r = future => {
                    let _ = output_tx.try_send(Ok(r));
                },
                _ = abort => {
                    if let Ok(_) =  output_tx.try_send(Err(Error::Aborted))  {
                        was_aborted.store(true, Ordering::SeqCst);
                    }
                 },
            };
        }
    });

    AsyncTask {
        abort_tx,
        output_rx,
        was_aborted,
    }
}
