use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use axum_extra::TypedHeader;
use futures::{SinkExt, StreamExt};
use once_cell::sync::Lazy;


use indi::client::ChangeError;
use logic::start;
use twinkle_api::flats::*;
use twinkle_client::{agent::Agent, task::Joinable};
use twinkle_client::{notify::Notify, task::Abortable};

use crate::{
    telescope::{DeviceError, OpticsConfig, Telescope, TelescopeConfig, TelescopeError},
    AppState,
};
// Global broadcast channel for trace events
pub static TRACE_CHANNEL: Lazy<tokio::sync::broadcast::Sender<String>> = Lazy::new(|| {
    // Configure a reasonable buffer size (adjust as needed)
    tokio::sync::broadcast::channel::<String>(1000).0
});

mod logic;

#[derive(Debug)]
pub enum FlatError {
    DeviceError(DeviceError),
    MissingBlob,
    FitsError(fitsrs::error::Error),
    TelescopeError(TelescopeError<()>),
    ChangeError(ChangeError<()>),
    SendError(tokio::sync::mpsc::error::SendError<twinkle_api::flats::MessageToClient>),
    SerdeError(serde_json::Error),
    AxumError(axum::Error),
    UnexpectedMessage,
    RecvError(tokio::sync::broadcast::error::RecvError),
    IoError(std::io::Error),
}

impl From<std::io::Error> for FlatError {
    fn from(value: std::io::Error) -> Self {
        FlatError::IoError(value)
    }
}
impl From<tokio::sync::broadcast::error::RecvError> for FlatError {
    fn from(value: tokio::sync::broadcast::error::RecvError) -> Self {
        FlatError::RecvError(value)
    }
}
impl From<axum::Error> for FlatError {
    fn from(value: axum::Error) -> Self {
        FlatError::AxumError(value)
    }
}
impl From<serde_json::Error> for FlatError {
    fn from(value: serde_json::Error) -> Self {
        FlatError::SerdeError(value)
    }
}
impl From<tokio::sync::mpsc::error::SendError<twinkle_api::flats::MessageToClient>> for FlatError {
    fn from(
        value: tokio::sync::mpsc::error::SendError<twinkle_api::flats::MessageToClient>,
    ) -> Self {
        FlatError::SendError(value)
    }
}

impl From<ChangeError<()>> for FlatError {
    fn from(value: ChangeError<()>) -> Self {
        FlatError::ChangeError(value)
    }
}

impl From<TelescopeError<()>> for FlatError {
    fn from(value: TelescopeError<()>) -> Self {
        FlatError::TelescopeError(value)
    }
}
impl From<DeviceError> for FlatError {
    fn from(value: DeviceError) -> Self {
        FlatError::DeviceError(value)
    }
}

impl From<fitsrs::error::Error> for FlatError {
    fn from(value: fitsrs::error::Error) -> Self {
        FlatError::FitsError(value)
    }
}

pub fn routes(router: Router<AppState>) -> Router<AppState> {
    router.route("/flats", get(create_connection))
}

#[tracing::instrument(skip_all)]
async fn create_connection(
    ws: WebSocketUpgrade,
    TypedHeader(host): TypedHeader<axum_extra::headers::Host>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let flats = state.store.read().await.flats.clone();
    Ok(ws.on_upgrade(move |socket| async move {
        handle_connection(socket, host, flats).await;
    }))
}

#[tracing::instrument(skip_all)]
async fn handle_connection(
    socket: WebSocket,
    _host: axum_extra::headers::Host,
    task_status: Arc<Notify<Agent<twinkle_api::flats::FlatRun>>>,
) {
    let telescope = Arc::new(
        Telescope::new(
            "192.168.8.197:7624",
            TelescopeConfig {
                mount: String::from("EQMod Mount"),
                primary_optics: OpticsConfig {
                    focal_length: 800.0,
                    aperture: 203.0,
                },
                primary_camera: String::from("ZWO CCD ASI294MM Pro"),
                focuser: String::from("ZWO EAF"),
                filter_wheel: String::from("ZWO EFW"),
                flat_panel: String::from("Deep Sky Dad FP"),
            },
            // "indi:7624",
            // TelescopeConfig {
            //     mount: String::from("Telescope Simulator"),
            //     primary_optics: OpticsConfig {
            //         focal_length: 800.0,
            //         aperture: 203.0,
            //     },
            //     primary_camera: String::from("CCD Simulator"),
            //     focuser: String::from("Focuser Simulator"),
            //     filter_wheel: String::from("Filter Simulator"),
            //     flat_panel: String::from("Light Panel Simulator"),
            // },
        )
        .await,
    );

    let (mut ws_write, mut ws_read) = socket.split();
    let (message_tx, mut message_rx) =
        tokio::sync::mpsc::channel::<twinkle_api::flats::MessageToClient>(10);

    let log_sender = {
        let mut sub = TRACE_CHANNEL.subscribe();
        let message_tx = message_tx.clone();

        async move {
            loop  {
                message_tx.send(MessageToClient::Log(sub.recv().await?)).await?;
            }
        }
    };
    let task_status_sender = {
        let mut task_status_sub = task_status.read().await.subscribe().await;
        let message_tx = message_tx.clone();
        async move {
            while let Some(Ok(task_status)) = task_status_sub.next().await {
                message_tx.send(MessageToClient::Status(task_status)).await?;
            }
            Result::<(), FlatError>::Ok(())
        }
    };

    let websocket_sender = {
        async move {
            while let Some(msg) = message_rx.recv().await {
                ws_write
                    .send(Message::Text(serde_json::to_string(&msg).unwrap()))
                    .await?;
                
            }
            Result::<(), FlatError>::Ok(())
        }
    };

    let websocket_receiver = {
        let task_status = task_status.clone();
        let telescope = telescope.clone();
        async move {
            while let Some(Ok(msg)) = ws_read.next().await {
                match msg {
                    Message::Text(msg) => {
                        match serde_json::from_str::<twinkle_api::flats::MessageToServer>(
                            msg.as_str(),
                        )? {
                            twinkle_api::flats::MessageToServer::Start(config) => {
                                let mut lock = task_status.write().await;
                                lock.abort();
                                let _ = lock.join().await;
                                lock.spawn(FlatRun { progress: 0. }, |state| {
                                    let telescope = telescope.clone();
                                    async move {
                                        if let Err(e) = start(telescope, config, state).await {
                                            tracing::error!("Error getting flats: {:?}", e);
                                        }
                                    }
                                });
                            }
                            twinkle_api::flats::MessageToServer::Stop => {
                                task_status.write().await.abort();
                            }
                        }
                    }
                    _ => {
                        Err(FlatError::UnexpectedMessage)?
                    }
                }
            }
            Result::<(), FlatError>::Ok(())
        }
    };

    let param_sender = {
        let telescope = telescope.clone();

        async move {
            let filter_wheel = telescope.get_filter_wheel().await?;
            let filters = filter_wheel.filters().await?;

            let mut filters_subscription = filters.subscribe().await;
            let mut params = Parameterization::default();
            params.binnings = vec![1, 2, 4];
            message_tx
                .send(twinkle_api::flats::MessageToClient::Parameterization(
                    params.clone(),
                ))
                .await?;
            while let Some(Ok(filters)) = filters_subscription.next().await {

                params.filters = filters.get()?.into_iter().map(|x| x.into()).collect();
                message_tx
                    .send(twinkle_api::flats::MessageToClient::Parameterization(
                        params.clone(),
                    ))
                    .await?;
            }
            Result::<(), FlatError>::Ok(())
        }
    };
    drop(task_status);
    if let Err(e) = tokio::select!{
        v = param_sender => v,
        v = websocket_sender => v,
        v = websocket_receiver => v,
        v = task_status_sender => v,
        v = log_sender => v,
    } {
        tracing::error!("Got error processing flats websocket: {:?}", e);
    }
}

#[cfg(test)]
mod test {}
