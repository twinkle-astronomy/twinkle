//! # A general purpose library for interacting with phd2.
//! PHD2 is telescope guiding software that simplifies the process of tracking a guide
//! star, letting you concentrate on other aspects of deep-sky imaging or spectroscopy.
//! For more information on phd2 see the project's website [here](https://openphdguiding.org/).
//!
//! The purpose of this crate is to provide a convinent way to interact with phd2 using the
//! using the EventMonitoring protocol.  Details on the protocol can be found [here](https://github.com/OpenPHDGuiding/phd2/wiki/EventMonitoring).
//!
//! ### Simple usage.
//!
//! The simpliest way to use this crate is to convert a [TcpStream](tokio::net::TcpStream) to a [Phd2Connection](Phd2Connection) to send commands and receive events.
//! #### Example
//! ```no_run
//! use phd2::{serialization::Event, Phd2Connection};
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut phd2: Phd2Connection<_>= tokio::net::TcpStream::connect("localhost:4400")
//!         .await
//!         .expect("Connecting to phd2")
//!         .into();
//!     let mut pixel_scale = phd2.get_pixel_scale().await.expect("Getting pixel scale.");
//!     
//!     let mut sub = phd2.subscribe().await;
//!
//!     while let Ok(event) = sub.recv().await {
//!         if let Event::GuideStep(guide) = &event.event {
//!             let delta = pixel_scale * (guide.dx.powi(2) + guide.dy.powi(2)).sqrt();
//!             println!("guide event: {} arcsec.", delta);
//!         }
//!         if let Event::ConfigurationChange(_) = &event.event {
//!             pixel_scale = phd2.get_pixel_scale().await.expect("Getting pixel scale.");
//!         }
//!     }
//! }
//! ```

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
    time::error::Elapsed,
};

#[cfg(test)]
mod tests;

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

impl<T: Send + tokio::io::AsyncRead + tokio::io::AsyncWrite + 'static> Phd2Connection<T> {
    pub fn from(value: T) -> (Phd2Connection<T>, tokio::sync::mpsc::Receiver<ServerEvent>) {
        let (read, write) = tokio::io::split(value);
        let (events, recv) = tokio::sync::mpsc::channel(1024);

        let client = Phd2Connection {
            connection: Arc::new(tokio::sync::Mutex::new(Connection {
                pending_requests: Default::default(),
                write,
            })),
            last_id: std::sync::atomic::AtomicU64::new(0),
        };

        let connection = client.connection.clone();

        tokio::spawn(async move {
            let mut read = BufReader::new(read);

            let mut buf = String::new();
            loop {
                buf.clear();
                if read.read_line(&mut buf).await.unwrap() == 0 {
                    break;
                }
                let obj = serde_json::from_str::<ServerMessage>(&buf);

                match obj {
                    Ok(obj) => match obj {
                        ServerMessage::ServerEvent(event) => {
                            events
                                .send(event)
                                .await
                                .expect("Sending ServerEvent to channel");
                        }
                        ServerMessage::JsonRpcResponse(rpc) => {
                            let mut lock = connection.lock().await;
                            if let Some(pr) = lock.pending_requests.remove(&rpc.id) {
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

        (client, recv)
    }
}

struct Connection<T> {
    pending_requests: HashMap<u64, tokio::sync::oneshot::Sender<JsonRpcResponse>>,
    write: tokio::io::WriteHalf<T>,
}

pub struct Phd2Connection<T> {
    connection: Arc<tokio::sync::Mutex<Connection<T>>>,

    last_id: std::sync::atomic::AtomicU64,
}

impl<T: Send + tokio::io::AsyncRead + tokio::io::AsyncWrite> Phd2Connection<T> {
    async fn call(&self, request: JsonRpcRequest) -> Result<serde_json::Value, ClientError> {
        Ok(tokio::time::timeout(Duration::from_secs(1), async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            {
                let mut sender = self.connection.lock().await;
                sender.pending_requests.insert(request.id, tx);
                sender.write.write(&serde_json::to_vec(&request)?).await?;
                sender.write.write(b"\n").await?;
            }
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

    fn next_id(&self) -> u64 {
        self.last_id.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn disconnect(self) -> std::io::Result<()> {
        let mut lock = self.connection.lock().await;
        lock.write.shutdown().await
    }
    pub async fn capture_single_frame(
        &self,
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
        &self,
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
        &self,
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

    pub async fn find_star(&self, roi: Option<[usize; 4]>) -> Result<[f64; 2], ClientError> {
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

    pub async fn flip_calibration(&self) -> Result<isize, ClientError> {
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

    pub async fn get_algo_param_names(&self, axis: Axis) -> Result<Vec<String>, ClientError> {
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
        &self,
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

    pub async fn get_app_state(&self) -> Result<State, ClientError> {
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

    pub async fn get_camera_frame_size(&self) -> Result<[usize; 2], ClientError> {
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

    pub async fn get_calibrated(&self) -> Result<bool, ClientError> {
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
        &self,
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

    pub async fn get_connected(&self) -> Result<bool, ClientError> {
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

    pub async fn get_cooler_status(&self) -> Result<CoolerStatus, ClientError> {
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

    pub async fn get_current_equipment(&self) -> Result<HashMap<String, Equipment>, ClientError> {
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
    pub async fn get_dec_guide_mode(&self) -> Result<DecGuideMode, ClientError> {
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
    pub async fn get_exposure(&self) -> Result<Duration, ClientError> {
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

    pub async fn get_exposure_durations(&self) -> Result<Vec<Duration>, ClientError> {
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

    pub async fn get_guide_output_enabled(&self) -> Result<bool, ClientError> {
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

    pub async fn get_lock_position(&self) -> Result<Option<[f64; 2]>, ClientError> {
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

    pub async fn get_lock_shift_enabled(&self) -> Result<bool, ClientError> {
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

    pub async fn get_lock_shift_params(&self) -> Result<LockShiftParams, ClientError> {
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

    pub async fn get_paused(&self) -> Result<bool, ClientError> {
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

    pub async fn get_pixel_scale(&self) -> Result<f64, ClientError> {
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

    pub async fn get_profile(&self) -> Result<Profile, ClientError> {
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

    pub async fn get_profiles(&self) -> Result<Vec<Profile>, ClientError> {
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

    pub async fn get_search_region(&self) -> Result<isize, ClientError> {
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

    pub async fn get_ccd_temperature(&self) -> Result<HashMap<String, f64>, ClientError> {
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
    /// PR to resolve this issue: https://github.com/OpenPHDGuiding/phd2/pull/1076
    pub async fn get_star_image(&self) -> Result<StarImage, ClientError> {
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

    pub async fn get_use_subframes(&self) -> Result<bool, ClientError> {
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
        &self,
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
        &self,
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
    pub async fn loop_(&self) -> Result<isize, ClientError> {
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

    pub async fn save_image(&self) -> Result<HashMap<String, String>, ClientError> {
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
        &self,
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
    pub async fn set_connected(&self, connected: bool) -> Result<isize, ClientError> {
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
    pub async fn set_dec_guide_mode(&self, mode: DecGuideMode) -> Result<isize, ClientError> {
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
    pub async fn set_exposure(&self, exposure: Duration) -> Result<isize, ClientError> {
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
    pub async fn set_guide_output_enabled(&self, enabled: bool) -> Result<isize, ClientError> {
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
        &self,
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

    pub async fn set_lock_shift_enabled(&self, enabled: bool) -> Result<isize, ClientError> {
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
        &self,
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
    pub async fn set_paused(&self, paused: bool, full: bool) -> Result<isize, ClientError> {
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
    pub async fn set_profile(&self, profile_id: isize) -> Result<isize, ClientError> {
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
    pub async fn shutdown(&self) -> Result<isize, ClientError> {
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
    pub async fn stop_capture(&self) -> Result<isize, ClientError> {
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
