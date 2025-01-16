use std::future::Future;

pub fn spawn<T: 'static, F: Future<Output = T> + 'static>(future: F) -> Task<T> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    wasm_bindgen_futures::spawn_local(async move {
        tx.send(future.await).ok();
    });
    Task { rx }
}

pub struct Task<T> {
    rx: tokio::sync::oneshot::Receiver<T>,
}

impl<T: 'static> Task<T> {
    pub async fn wait(self) -> Result<T, tokio::sync::oneshot::error::RecvError> {
        self.rx.await
    }
}
