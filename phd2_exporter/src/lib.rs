pub mod async_middleware;
pub mod metrics;
pub mod serialization;
use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc},
};

use serde::Deserialize;
use serde_json::{de::IoRead, Deserializer, StreamDeserializer};
use serialization::{
    InvalidState, JsonRpcRequest, JsonRpcResponse, ServerEvent, ServerMessage, State,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
};
pub trait WithMiddleware<T: std::io::Read> {
    fn middleware<F>(self, func: F) -> ReadMiddleware<T, F>
    where
        F: Fn(&[u8]);
}

impl<T: std::io::Read> WithMiddleware<T> for T {
    fn middleware<F>(self, func: F) -> ReadMiddleware<T, F>
    where
        F: Fn(&[u8]),
    {
        ReadMiddleware { read: self, func }
    }
}

pub struct ReadMiddleware<T, F>
where
    T: std::io::Read,
    F: Fn(&[u8]),
{
    read: T,
    func: F,
}

impl<T, F> ReadMiddleware<T, F>
where
    T: std::io::Read,
    F: Fn(&[u8]),
{
    pub fn new(read: T, func: F) -> Self {
        ReadMiddleware { read, func }
    }
}

impl<T, F> std::io::Read for ReadMiddleware<T, F>
where
    T: std::io::Read,
    F: Fn(&[u8]),
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = self.read.read(buf)?;
        (self.func)(&buf[0..len]);
        Ok(len)
    }
}

pub trait Connection {
    type Iter<T: Deserialize<'static>>: Iterator<Item = serde_json::Result<T>>;

    fn iter<T: Deserialize<'static>>(self) -> Self::Iter<T>;

    // fn write<T: Serialize>(&mut self, message: &T) -> serde_json::Result<()>
    // where
    //     Self: Sized + std::io::Write,{
    //     serde_json::to_writer(self, message)
    // }
}

impl<T: std::io::Read> Connection for T {
    type Iter<I: Deserialize<'static>> = StreamDeserializer<'static, IoRead<Self>, I>;

    fn iter<I: Deserialize<'static>>(self) -> Self::Iter<I> {
        Deserializer::from_reader(self).into_iter::<I>()
    }
}

// pub struct ItemStream<T, I> {
//     reader: T,
//     buf: String,
//     _phantom: I
// }

// impl<T: tokio::io::AsyncBufRead + std::marker::Unpin, I: Deserialize<'static>> Stream for ItemStream<T, I> {
//     type Item = Result<I, serde_json::Error>;

//     fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
//         let r = (*self).reader.read_line(**self.buf);
//         // self.reader.read_line(&mut self.buf);
//         todo!()
//     }
// }

// pub trait AsyncConnection {
//     type Stream<T: Deserialize<'static>>: Stream<Item=serde_json::Result<T>>;

//     fn stream<T: Deserialize<'static>>(self) -> Self::Stream<T>;
// }

// impl<T: tokio::io::AsyncBufRead + std::marker::Unpin> AsyncConnection for T {
//     type Stream<I: Deserialize<'static>> = ItemStream<T, I>;

//     fn stream<I: Deserialize<'static>>(self) -> Self::Stream<I> {
//         todo!()
//     }
// }

#[derive(Debug)]
pub enum ClientError {
    IoError(std::io::Error),
    SerdeJsonError(serde_json::Error),
    RpcError(serde_json::Value),
    RpcUnexpectedResponse(serde_json::Value),
    RpcMissingResult,
    InvalidState(InvalidState),
}

impl From<InvalidState> for ClientError {
    fn from(value: InvalidState) -> Self {
        ClientError::InvalidState(value)
    }
}

impl From<std::io::Error> for ClientError {
    fn from(value: std::io::Error) -> Self {
        ClientError::IoError(value)
    }
}
impl From<serde_json::Error> for ClientError {
    fn from(value: serde_json::Error) -> Self {
        ClientError::SerdeJsonError(value)
    }
}

pub trait IntoPhd2Connection {
    fn phd2(self) -> Phd2Connection<Self>
    where
        Self: Sized;
}
impl<T: Send + tokio::io::AsyncRead + tokio::io::AsyncWrite + 'static> IntoPhd2Connection for T {
    fn phd2(self) -> Phd2Connection<T> {
        let (read, write) = tokio::io::split(self);
        let pending_requests = Arc::new(Mutex::new(HashMap::default()));
        let pending_responses = pending_requests.clone();
        let (events, _) = tokio::sync::broadcast::channel(1024);
        let client = Phd2Connection {
            events,
            pending_requests,
            write,
            last_id: std::sync::atomic::AtomicU64::new(0),
        };

        let new_events = client.events.clone();
        tokio::spawn(async move {
            let mut read = BufReader::new(read);

            let mut buf = String::new();
            loop {
                buf.clear();
                if read.read_line(&mut buf).await.unwrap() == 0 {
                    dbg!("break");
                    break;
                }
                let obj = serde_json::from_str::<ServerMessage>(&buf);

                match obj {
                    Ok(obj) => match obj {
                        ServerMessage::ServerEvent(event) => {
                            new_events.send(Arc::new(event)).ok();
                        }
                        ServerMessage::JsonRpcResponse(rpc) => {
                            dbg!(&rpc);
                            let mut lock = pending_responses.lock().await;
                            if let Some(pr) = lock.remove(&rpc.id) {
                                pr.send(rpc).ok();
                            }
                        }
                    },
                    Err(e) => {
                        dbg!(e);
                    }
                }
            }
        });

        client
    }
}

pub struct Phd2Connection<T> {
    events: tokio::sync::broadcast::Sender<Arc<ServerEvent>>,
    pending_requests: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<JsonRpcResponse>>>>,
    write: tokio::io::WriteHalf<T>,

    last_id: std::sync::atomic::AtomicU64,
}

impl<T: Send + tokio::io::AsyncRead + tokio::io::AsyncWrite> Phd2Connection<T> {
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Arc<ServerEvent>> {
        self.events.subscribe()
    }

    async fn call(&mut self, request: JsonRpcRequest) -> Result<serde_json::Value, ClientError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        dbg!(&request);
        {
            let mut lock = self.pending_requests.lock().await;
            lock.insert(request.id, tx);
        }
        self.write.write(&serde_json::to_vec(&request)?).await?;
        self.write.write(b"\n").await?;
        let resp = rx.await.unwrap();

        if let Some(e) = resp.error {
            return Err(ClientError::RpcError(e));
        }
        match resp.result {
            Some(result) => Ok(result),
            None => Err(ClientError::RpcMissingResult),
        }
    }

    fn next_id(&mut self) -> u64 {
        self.last_id.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn get_app_state(&mut self) -> Result<State, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_app_state"),
                params: vec![],
            })
            .await?;

        match result.as_str() {
            Some(s) => Ok(s.try_into()?),
            None => Err(ClientError::RpcUnexpectedResponse(result)),
        }
    }

    pub async fn get_calibrated(&mut self) -> Result<bool, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_calibrated"),
                params: vec![],
            })
            .await?;

        match result.as_bool() {
            Some(s) => Ok(s),
            None => Err(ClientError::RpcUnexpectedResponse(result)),
        }
    }

    pub async fn get_pixel_scale(&mut self) -> Result<f64, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_pixel_scale"),
                params: vec![],
            })
            .await?;

        match result.as_f64() {
            Some(s) => Ok(s),
            None => Err(ClientError::RpcUnexpectedResponse(result)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;

    #[test]
    fn test_read_session() {
        let file = File::open("./src/test_data/session.log").unwrap();

        for _event in file.iter::<ServerEvent>() {
            dbg!(_event.unwrap());
        }
    }
}
