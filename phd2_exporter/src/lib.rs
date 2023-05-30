pub mod metrics;
pub mod serialization;
use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use serde::Serialize;
use serde_json::json;
use serialization::{
    DurationSeconds, InvalidState, JsonRpcRequest, JsonRpcResponse, ServerEvent, ServerMessage,
    State,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
};

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

impl<T: Send + tokio::io::AsyncRead + tokio::io::AsyncWrite + 'static> From<T>
    for Phd2Connection<T>
{
    fn from(value: T) -> Self {
        let (read, write) = tokio::io::split(value);
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

#[derive(Serialize, Debug)]
pub enum ClearCalibrationParam {
    #[serde(rename = "mount")]
    Mount,
    #[serde(rename = "ao")]
    Ao,
    #[serde(rename = "both")]
    Both,
}

#[derive(Serialize, Debug)]
pub struct Settle {
    pub pixels: f64,

    pub time: DurationSeconds,

    pub timeout: DurationSeconds,
}
impl Settle {
    pub fn new(pixels: f64, time: Duration, timeout: Duration) -> Settle {
        Settle {
            pixels,
            time: time.into(),
            timeout: timeout.into(),
        }
    }
}
impl<T: Send + tokio::io::AsyncRead + tokio::io::AsyncWrite> Phd2Connection<T> {
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Arc<ServerEvent>> {
        self.events.subscribe()
    }

    async fn call(&mut self, request: JsonRpcRequest) -> Result<serde_json::Value, ClientError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
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

    pub async fn capture_single_frame(
        &mut self,
        exposure: Duration,
        subframe: Option<[u32; 4]>,
    ) -> Result<i64, ClientError> {
        let id = self.next_id();
        let mut params = json!({"exposure": exposure.as_secs()});
        if let Some(subframe) = subframe {
            params["subframe"] = json!(subframe);
        }
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("capture_single_frame"),
                params: params,
            })
            .await?;

        match result.as_i64() {
            Some(s) => Ok(s),
            None => Err(ClientError::RpcUnexpectedResponse(result)),
        }
    }

    pub async fn clear_calibration(
        &mut self,
        target: ClearCalibrationParam,
    ) -> Result<i64, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("clear_calibration"),
                params: json!([target]),
            })
            .await?;

        match result.as_i64() {
            Some(s) => Ok(s),
            None => Err(ClientError::RpcUnexpectedResponse(result)),
        }
    }

    pub async fn dither(
        &mut self,
        amount: f64,
        ra_only: bool,
        settle: Settle,
    ) -> Result<i64, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("dither"),
                params: json!({"amount": amount, "raOnly": ra_only, "settle": settle}),
            })
            .await?;

        match result.as_i64() {
            Some(s) => Ok(s),
            None => Err(ClientError::RpcUnexpectedResponse(result)),
        }
    }

    pub async fn find_star(&mut self, roi: Option<[u64; 4]>) -> Result<bool, ClientError> {
        let id = self.next_id();
        let mut params = json!({});

        if let Some(roi) = roi {
            params["roi"] = serde_json::to_value(roi).unwrap();
        }

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("find_star"),
                params: params,
            })
            .await?;

        match result.as_bool() {
            Some(s) => Ok(s),
            None => Err(ClientError::RpcUnexpectedResponse(result)),
        }
    }

    pub async fn get_app_state(&mut self) -> Result<State, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_app_state"),
                params: json!([]),
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
                params: json!([]),
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
                params: json!([]),
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

    use tokio::fs::File;

    #[tokio::test]
    async fn test_read_session() {
        let file: Phd2Connection<File> = File::open("./src/test_data/session.log")
            .await
            .unwrap()
            .into();
        let mut sub = file.subscribe();
        while let Ok(event) = sub.recv().await {
            dbg!(event);
        }
    }
}
