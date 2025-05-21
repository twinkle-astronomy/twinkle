use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::StreamExt;
use once_cell::sync::Lazy;

use indi::{client::{active_device::SendError, ChangeError}, serialization::Command};
use logic::start;
use tokio_stream::wrappers::ReceiverStream;
use twinkle_api::flats::*;
use twinkle_client::task::Abortable;
use twinkle_client::task::Joinable;

use crate::{
    telescope::{Connectable, DeviceError, Telescope, TelescopeError}, websocket_handler::WebsocketHandler, AppState
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
    SendError(tokio::sync::mpsc::error::SendError<MessageToClient>),
    SendCommandError(SendError<Command>),
    SerdeError(serde_json::Error),
    AxumError(axum::Error),
    UnexpectedMessage,
    RecvError(tokio::sync::broadcast::error::RecvError),
    IoError(std::io::Error),
}

impl From<SendError<Command>> for FlatError {
    fn from(value: SendError<Command>) -> Self {
        FlatError::SendCommandError(value)
    }
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
impl From<tokio::sync::mpsc::error::SendError<MessageToClient>> for FlatError {
    fn from(value: tokio::sync::mpsc::error::SendError<MessageToClient>) -> Self {
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

pub fn routes() -> Router<AppState> {
    Router::new().route("/flats", get(create_connection))
}

#[tracing::instrument(skip_all)]
async fn create_connection(
    ws: WebSocketUpgrade,
    state: State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    Ok(ws.on_upgrade(move |socket| async move {
        handle_connection(socket.into(), state).await;
    }))
}

#[tracing::instrument(skip_all)]
async fn handle_connection(mut socket: WebsocketHandler, State(state): State<AppState>) {
    let store = state.store.read().await;
    let task_status = store.flats.clone();
    let settings = store.settings.read().await;
    
    let telescope = Arc::new(
        Telescope::new(
            settings.indi_server_addr.clone(),
            settings.telescope_config.clone(),
        )
        .await,
    );
    drop(settings);
    drop(store);
    
    let (message_tx, message_rx) = tokio::sync::mpsc::channel::<MessageToClient>(10);

    let (from_websocket_tx, from_websocket_rx) = tokio::sync::mpsc::channel::<Message>(10);

    let log_sender = {
        let mut sub = TRACE_CHANNEL.subscribe();
        let message_tx = message_tx.clone();

        async move {
            while let Ok(log) = sub.recv().await {
                message_tx.send(MessageToClient::Log(log)).await?;
            }
            Result::<(), FlatError>::Ok(())
        }
    };
    let task_status_sender = {
        let mut task_status_sub = task_status.read().await.subscribe().await;
        let message_tx = message_tx.clone();
        async move {
            while let Some(Ok(task_status)) = task_status_sub.next().await {
                let message_status = task_status.map(|x| x.map(|y| y.as_ref().clone()));

                message_tx.send(MessageToClient::Status(message_status)).await?;
            }
            Result::<(), FlatError>::Ok(())
        }
    };

    let websocket_receiver = {
        let task_status = task_status.clone();
        let telescope = telescope.clone();
        let mut from_websocket_rx = ReceiverStream::new(from_websocket_rx);
        async move {
            while let Some(msg) = from_websocket_rx.next().await {
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
                    _ => Err(FlatError::UnexpectedMessage)?,
                }
            }
            Result::<(), FlatError>::Ok(())
        }
    };

    let param_sender = {
        let telescope = telescope.clone();

        async move {
            let filter_wheel = telescope.get_filter_wheel().await?;
            let _ = filter_wheel.connect().await?;

            let filters = filter_wheel.filters().await?;

            let mut filters_subscription = filters.subscribe().await;

            let mut params = Parameterization::default();
            params.binnings = vec![1, 2, 4];
            while let Some(Ok(filters)) = filters_subscription.next().await {
                params.filters = filters.get()?.into_iter().map(|x| x.into()).collect();
                message_tx
                    .send(twinkle_api::flats::MessageToClient::Parameterization(params.clone()))
                    .await?;
            }
            Result::<(), FlatError>::Ok(())
        }
    };
    drop(task_status);

    socket.set_sender(from_websocket_tx);
    
    let websocket_handler_future = async move {
        socket.handle_websocket_stream(ReceiverStream::new(message_rx)).await; Result::<(), FlatError>::Ok(())
    };

    tokio::select! {
        v = param_sender => {
            if let Err(e) = v {
                 tracing::error!("Error in param_sender: {:?}", e);
            }
        },
        v = websocket_receiver => {
            if let Err(e) = v {
                 tracing::error!("Error in websocket_receiver: {:?}", e);
            }
        },

        v = task_status_sender => {
            if let Err(e) = v {
                 tracing::error!("Error in task_status_sender: {:?}", e);
            }
        },

        v = log_sender => {
            if let Err(e) = v {
                 tracing::error!("Error in log_sender: {:?}", e);
            }
        },

        v = websocket_handler_future => {
            if let Err(e) = v {
                 tracing::error!("Error in websocket_handler_future: {:?}", e);
            }
        },

    }
}

