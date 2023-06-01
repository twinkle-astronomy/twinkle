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
    Axis, Calibration, ClearCalibrationParam, CoolerStatus, DecGuideMode, DurationMillis,
    Equipment, InvalidState, JsonRpcRequest, JsonRpcResponse, LockShiftParams, Profile,
    PulseDirection, ServerEvent, ServerMessage, Settle, StarImage, State, WhichDevice,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
    time::error::Elapsed,
};
use tokio_util::io::{InspectReader, InspectWriter};

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
        let events = Arc::new(Mutex::new(Some(events)));
        let new_events = events.clone();
        let client = Phd2Connection {
            events,
            pending_requests,
            write,
            last_id: std::sync::atomic::AtomicU64::new(0),
        };

        tokio::spawn(async move {
            let mut read = BufReader::new(read);

            let mut buf = String::new();
            loop {
                buf.clear();
                if read.read_line(&mut buf).await.unwrap() == 0 {
                    let mut lock = new_events.lock().await;
                    *lock = None;
                    break;
                }
                let obj = serde_json::from_str::<ServerMessage>(&buf);

                match obj {
                    Ok(obj) => match obj {
                        ServerMessage::ServerEvent(event) => {
                            let lock = new_events.lock().await;
                            if let Some(sender) = lock.as_ref() {
                                sender.send(Arc::new(event)).ok();
                            }
                        }
                        ServerMessage::JsonRpcResponse(rpc) => {
                            let mut lock = pending_responses.lock().await;
                            if let Some(pr) = lock.remove(&rpc.id) {
                                pr.send(rpc).ok();
                            }
                        }
                    },
                    Err(e) => {
                        dbg!(&buf);
                        dbg!(e);
                    }
                }
            }
        });

        client
    }
}

pub struct Phd2Connection<T> {
    events: Arc<Mutex<Option<tokio::sync::broadcast::Sender<Arc<ServerEvent>>>>>,
    pending_requests: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<JsonRpcResponse>>>>,
    write: tokio::io::WriteHalf<T>,

    last_id: std::sync::atomic::AtomicU64,
}

impl<T: Send + tokio::io::AsyncRead + tokio::io::AsyncWrite> Phd2Connection<T> {
    pub async fn subscribe(&self) -> Option<tokio::sync::broadcast::Receiver<Arc<ServerEvent>>> {
        let lock = self.events.lock().await;
        Some(lock.as_ref()?.subscribe())
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
    ) -> Result<isize, ClientError> {
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
    ) -> Result<isize, ClientError> {
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
    ) -> Result<isize, ClientError> {
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

    pub async fn find_star(&mut self, roi: Option<[usize; 4]>) -> Result<[f64; 2], ClientError> {
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

    pub async fn flip_calibration(&mut self) -> Result<isize, ClientError> {
        let id = self.next_id();

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

    pub async fn get_camera_frame_size(&mut self) -> Result<[usize; 2], ClientError> {
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

    pub async fn get_profiles(&mut self) -> Result<Vec<Profile>, ClientError> {
        let id = self.next_id();
        let result = self
            .call(JsonRpcRequest {
                id,
                method: String::from("get_profiles"),
                params: json!([]),
            })
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_search_region(&mut self) -> Result<isize, ClientError> {
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

    pub async fn get_ccd_temperature(&mut self) -> Result<HashMap<String, f64>, ClientError> {
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

pub trait WithInspectReader<R: tokio::io::AsyncRead> {
    fn inspect_read<F>(self, func: F) -> InspectReader<R, F>
    where
        F: FnMut(&[u8]);
}

impl<T: tokio::io::AsyncRead> WithInspectReader<T> for T {
    fn inspect_read<F>(self, func: F) -> InspectReader<T, F>
    where
        F: FnMut(&[u8]),
    {
        InspectReader::new(self, func)
    }
}

pub trait WithInspectWriter<R: tokio::io::AsyncWrite> {
    fn inspect_write<F>(self, func: F) -> InspectWriter<R, F>
    where
        F: FnMut(&[u8]);
}

impl<T: tokio::io::AsyncWrite> WithInspectWriter<T> for T {
    fn inspect_write<F>(self, func: F) -> InspectWriter<T, F>
    where
        F: FnMut(&[u8]),
    {
        InspectWriter::new(self, func)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::serialization::Event;
    use std::io::Write;

    use tokio::{fs::File, net::TcpStream};

    #[tokio::test]
    async fn test_read_session() {
        let file: Phd2Connection<File> = File::open("./src/test_data/session.log")
            .await
            .unwrap()
            .into();
        let mut sub = file.subscribe().await.unwrap();
        while let Ok(event) = sub.recv().await {
            dbg!(event);
        }
    }

    fn verbose_log(prefix: &str, buf: &[u8]) {
        std::io::stdout().write(prefix.as_bytes()).unwrap();
        std::io::stdout()
            .write(format!("{:?}", std::str::from_utf8(buf).unwrap()).as_bytes())
            .unwrap();
        std::io::stdout().write(&[b'\n']).unwrap();
    }

    #[cfg(feature = "test_phd2_simulator")]
    #[tokio::test]
    async fn test_integration_phd2_simulator() -> Result<(), ClientError> {
        println!("Starting phd2");

        let mut phd2_instance = tokio::process::Command::new("phd2").spawn().unwrap();

        let mut phd2: Phd2Connection<_> = loop {
            println!("Connecting to phd2");
            let connection = TcpStream::connect("localhost:4400").await;
            match connection {
                Ok(connection) => {
                    break connection
                        .inspect_read(move |buf: &[u8]| verbose_log("-> ", buf))
                        .inspect_write(move |buf: &[u8]| verbose_log("<- ", buf))
                        .into()
                }
                Err(e) => {
                    dbg!(e);
                    println!("Waiting 1s before trying again");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        };

        phd2.stop_capture().await?;
        phd2.set_connected(false).await?;

        let profiles = phd2.get_profiles().await?;
        let simulator_profile = profiles
            .iter()
            .find(|item| item.name == "Simulator")
            .expect("Finding 'Simulator' profile.");
        phd2.set_profile(simulator_profile.id).await?;

        assert_eq!(phd2.get_profile().await?.name, String::from("Simulator"));

        phd2.set_connected(true).await?;
        assert!(phd2.get_connected().await?);

        assert_eq!(phd2.get_app_state().await?, State::Stopped);

        phd2.clear_calibration(ClearCalibrationParam::Both).await?;
        assert!(!phd2.get_calibrated().await?);

        phd2.get_camera_frame_size().await?;
        phd2.get_current_equipment().await?;

        phd2.get_cooler_status().await?;

        phd2.set_dec_guide_mode(DecGuideMode::Auto).await?;
        assert_eq!(phd2.get_dec_guide_mode().await?, DecGuideMode::Auto);

        let exp = phd2.get_exposure_durations().await?[0];
        phd2.set_exposure(exp).await?;
        assert_eq!(phd2.get_exposure().await?, exp);

        phd2.set_guide_output_enabled(true).await?;
        assert!(phd2.get_guide_output_enabled().await?);

        phd2.get_lock_shift_params().await?;
        phd2.set_lock_shift_params([1.0, 1.0], "arcsec/hr", "RA/Dec")
            .await?;
        phd2.set_lock_shift_enabled(false).await?;
        assert!(!phd2.get_lock_shift_enabled().await?);

        let param = &phd2.get_algo_param_names(Axis::Dec).await?[1];
        let value = phd2.get_algo_param(Axis::Dec, param).await?;
        phd2.set_algo_param(Axis::Dec, param, value).await?;

        phd2.get_pixel_scale().await?;
        phd2.get_search_region().await?;
        phd2.get_ccd_temperature().await?;
        phd2.get_use_subframes().await?;

        // Start doing frame-things
        phd2.capture_single_frame(Duration::from_secs(1), None)
            .await?;

        phd2.set_exposure(Duration::from_secs(1)).await?;
        {
            println!("Starting looping");
            let mut events = phd2.subscribe().await.expect("Getting events");
            phd2.loop_().await?;

            let mut frame_count = 0;
            loop {
                dbg!(frame_count);
                let event = events.recv().await.unwrap();
                if let Event::LoopingExposures(_) = &event.event {
                    frame_count += 1;
                    if frame_count > 5 {
                        break;
                    }
                }
            }
            phd2.guide_pulse(10, PulseDirection::E, None).await?;
            phd2.find_star(Some([621, 356, 50, 50])).await?;
        }
        {
            println!("Starting guiding");
            let settle = Settle::new(1.5, Duration::from_secs(1), Duration::from_secs(60));
            let mut events = phd2.subscribe().await.expect("Getting events");
            phd2.guide(settle, Some(true), None).await?;

            loop {
                let event = events.recv().await.unwrap();
                if let Event::SettleDone(event) = &event.event {
                    assert_eq!(event.status, 0);
                    break;
                }
            }

            assert!(phd2.get_calibrated().await?);
            phd2.get_calibration_data(WhichDevice::Mount).await?;
            phd2.set_guide_output_enabled(false).await?;
            assert!(!phd2.get_guide_output_enabled().await?);
            phd2.set_guide_output_enabled(true).await?;

            phd2.get_lock_position().await?;

            println!("Dither!");
            phd2.dither(10.0, false, settle).await?;

            loop {
                let event = events.recv().await.unwrap();
                if let Event::SettleDone(event) = &event.event {
                    assert_eq!(event.status, 0);
                    break;
                }
            }
            phd2.flip_calibration().await?;
            let pos = phd2.get_lock_position().await?.unwrap();
            phd2.set_lock_position(pos[0], pos[1], None).await?;
            phd2.set_paused(true, true).await?;
            assert!(phd2.get_paused().await?);

            phd2.stop_capture().await?;
        }
        phd2.shutdown().await?;

        let shutdown = tokio::time::timeout(Duration::from_secs(5), phd2_instance.wait()).await;

        if let Ok(Ok(status)) = shutdown {
            assert!(status.success());
        } else {
            dbg!(&shutdown);
            phd2_instance.kill().await.expect("Killing phd2");
            panic!("Shutting down phd2 didn't work");
        }
        Ok(())
    }
}
