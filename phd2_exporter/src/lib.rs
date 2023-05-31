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
    DurationMillis, InvalidState, JsonRpcRequest, JsonRpcResponse, ServerEvent,
    ServerMessage, State, ClearCalibrationParam, Settle, Axis, WhichDevice, Calibration, CoolerStatus, DecGuideMode, Equipment, LockShiftParams, Profile, StarImage, PulseDirection,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
    time::error::Elapsed,
};

#[derive(Debug)]
pub enum ClientError {
    IoError(std::io::Error),
    SerdeJsonError(serde_json::Error),
    RpcError(serde_json::Value),
    RpcUnexpectedResponse(serde_json::Value),
    RpcMissingResult,
    InvalidState(InvalidState),
    Timeout(Elapsed),
}

impl From<Elapsed> for ClientError {
    fn from(value: Elapsed) -> Self {
        ClientError::Timeout(value)
    }
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

impl<T: Send + tokio::io::AsyncRead + tokio::io::AsyncWrite> Phd2Connection<T> {
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Arc<ServerEvent>> {
        self.events.subscribe()
    }

    async fn call(&mut self, request: JsonRpcRequest) -> Result<serde_json::Value, ClientError> {
        Ok(tokio::time::timeout(Duration::from_secs(1), async move {
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
        })
        .await??)
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

        Ok(serde_json::from_value(result)?)
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

        Ok(serde_json::from_value(result)?)
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

        Ok(serde_json::from_value(result)?)
    }

    pub async fn find_star(&mut self, roi: Option<[u64; 4]>) -> Result<[f64; 2], ClientError> {
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
        Ok(serde_json::from_value(result)?)
    }

    pub async fn flip_calibration(&mut self, roi: Option<[u64; 4]>) -> Result<i64, ClientError> {
        let id = self.next_id();
        let mut params = json!({});

        if let Some(roi) = roi {
            params["roi"] = serde_json::to_value(roi).unwrap();
        }

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("flip_calibration"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_algo_param_names(&mut self, axis: Axis) -> Result<Vec<String>, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_algo_param_names"),
                params: json!([axis]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_algo_param<S: Into<String> + Serialize>(
        &mut self,
        axis: Axis,
        name: S,
    ) -> Result<f64, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_algo_param"),
                params: json!([axis, name]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
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

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_camera_frame_size(&mut self) -> Result<[u64; 2], ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_camera_frame_size"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
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

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_calibration_data(
        &mut self,
        which: WhichDevice,
    ) -> Result<Calibration, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_calibration_data"),
                params: json!([which]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_connected(&mut self) -> Result<bool, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_connected"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_cooler_status(&mut self) -> Result<CoolerStatus, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_cooler_status"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_current_equipment(
        &mut self,
    ) -> Result<HashMap<String, Equipment>, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_current_equipment"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn get_dec_guide_mode(&mut self) -> Result<DecGuideMode, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_dec_guide_mode"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn get_exposure(&mut self) -> Result<Duration, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_exposure"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value::<DurationMillis>(result)?.into())
    }

    pub async fn get_exposure_durations(&mut self) -> Result<Vec<Duration>, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_exposure_durations"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value::<Vec<DurationMillis>>(result)?
            .into_iter()
            .map(|x| x.into())
            .collect())
    }

    pub async fn get_guide_output_enabled(&mut self) -> Result<bool, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_guide_output_enabled"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_lock_position(&mut self) -> Result<Option<[f64; 2]>, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_lock_position"),
                params: json!([]),
            })
            .await;
        let result = match result {
            Ok(r) => r,
            Err(ClientError::RpcMissingResult) => return Ok(None),
            Err(e) => {
                return Err(e);
            }
        };

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_lock_shift_enabled(&mut self) -> Result<bool, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_lock_shift_enabled"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_lock_shift_params(&mut self) -> Result<LockShiftParams, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_lock_shift_params"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_paused(&mut self) -> Result<bool, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_paused"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
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

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_profile(&mut self) -> Result<Profile, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_profile"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_search_region(&mut self) -> Result<i64, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_search_region"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_ccd_temperature(&mut self) -> Result<f64, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_ccd_temperature"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    /// Phd2 simulator is giving me an invalid string for the pixels causing a parse error for the response.
    pub async fn get_star_image(&mut self) -> Result<StarImage, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_star_image"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_use_subframes(&mut self) -> Result<bool, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_use_subframes"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn guide(
        &mut self,
        settle: Settle,
        recalibrate: Option<bool>,
        roi: Option<[usize; 4]>,
    ) -> Result<isize, ClientError> {
        let id = self.next_id();
        let mut params = json!({ "settle": settle });
        if let Some(recalibrate) = recalibrate {
            params["recalibrate"] = serde_json::Value::Bool(recalibrate);
        }
        if let Some(roi) = roi {
            params["roi"] = serde_json::to_value(roi).unwrap();
        }
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("guide"),
                params,
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn guide_pulse(
        &mut self,
        amount: isize,
        direction: PulseDirection,
        which: Option<WhichDevice>,
    ) -> Result<isize, ClientError> {
        let id = self.next_id();
        let mut params = json!({"amount": amount, "direction": direction});
        if let Some(which) = which {
            params["which"] = serde_json::to_value(which).unwrap();
        }
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("guide_pulse"),
                params,
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    /// This is the `loop` rpc method, but functions in rust can't
    /// be named `loop`, so it's `loop_` instead.
    pub async fn loop_(&mut self) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("loop"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn save_image(&mut self) -> Result<HashMap<String, String>, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("save_image"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn set_algo_param(
        &mut self,
        axis: Axis,
        name: impl Into<String>,
        value: f64,
    ) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_algo_param"),
                params: json!([axis, name.into(), value]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn set_connected(&mut self, connected: bool) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_connected"),
                params: json!([connected]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn set_dec_guide_mode(&mut self, mode: DecGuideMode) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_dec_guide_mode"),
                params: json!([mode]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn set_exposure(&mut self, exposure: Duration) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_exposure"),
                params: json!([DurationMillis(exposure)]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn set_guide_output_enabled(&mut self, enabled: bool) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_guide_output_enabled"),
                params: json!([enabled]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn set_lock_position(
        &mut self,
        x: f64,
        y: f64,
        exact: Option<bool>,
    ) -> Result<isize, ClientError> {
        let id = self.next_id();
        let mut params = json!({"x": x, "y": y});
        if let Some(exact) = exact {
            params["exact"] = serde_json::Value::Bool(exact);
        }

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_lock_position"),
                params: params,
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn set_lock_shift_enabled(&mut self, enabled: bool) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_lock_shift_enabled"),
                params: json!([enabled]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn set_lock_shift_params(
        &mut self,
        rate: [f64; 2],
        units: impl Into<String>,
        axes: impl Into<String>,
    ) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_lock_shift_params"),
                params: json!({
                    "rate": rate,
                    "units": units.into(),
                    "axes": axes.into()
                }),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn set_paused(&mut self, paused: bool, full: bool) -> Result<isize, ClientError> {
        let id = self.next_id();
        let mut params = json!({ "paused": paused });

        if full {
            params["type"] = serde_json::Value::String(String::from("full"));
        }

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_paused"),
                params,
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn set_profile(&mut self, profile_id: isize) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("set_profile"),
                params: json!([profile_id]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn shutdown(&mut self) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("shutdown"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }
    pub async fn stop_capture(&mut self) -> Result<isize, ClientError> {
        let id = self.next_id();

        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("stop_capture"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
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
