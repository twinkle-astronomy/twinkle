use std::time::Duration;
use std::{ops::Deref, sync::Arc};

use axum::http::StatusCode;
use axum::{
    extract::{ws::Message, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures::{self, stream, Stream, StreamExt, TryStreamExt};
use indi::serialization::Sexagesimal;
use indi::Number;
use twinkle_api::capture::{
    CaptureProgress, CaptureRequest, ExposureParameterization, MessageToClient,
};
use twinkle_api::ToWebsocketMessage;
use twinkle_client::{
    notify::NotifyArc,
    task::{Abortable, Status, TaskStatusError},
};

use crate::{telescope::Telescope, websocket_handler::WebsocketHandler, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/capture", get(get_capture))
        .route("/capture", post(set_capture))
}

#[tracing::instrument(skip(state))]
pub async fn set_capture(
    State(state): State<AppState>,
    Json(capture): Json<CaptureRequest>,
) -> impl IntoResponse {
    let mut store = state.store.write().await;
    let telescope = Arc::new(Telescope::new_from_settings(&store.settings).await);

    match capture {
        CaptureRequest::Start(capture_config) => {
            store.capture.abort();
            store
                .capture
                .spawn(CaptureProgress { progress: 0. }, move |state| async move {
                    let camera = telescope.get_primary_camera().await.unwrap();
                    let exposure = camera.exposure().await.unwrap();

                    let exposure_secs = capture_config.exposure.as_secs_f64();
                    let mut stream = exposure.subscribe().await;
                    while let Some(value) = stream.try_next().await.unwrap() {
                        let remaining: Sexagesimal = value.get().unwrap().into();
                        if f64::from(remaining) == 0. {
                            exposure.set(capture_config.exposure.as_secs_f64()).unwrap();
                        }
                        let progress = 1. - f64::from(remaining) / exposure_secs;
                        state.write().await.progress = progress;
                    }
                });
        }
        CaptureRequest::Stop => {
            store.capture.abort();
        }
    }
    StatusCode::OK
}

#[tracing::instrument(skip_all)]
async fn get_capture(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.store.read().await;
    let capture = store.capture.subscribe().await;
    let telescope = Arc::new(Telescope::new_from_settings(&store.settings).await);

    drop(store);
    Ok(ws.on_upgrade(move |socket| async move {
        let exposure: Number = telescope
            .get_primary_camera()
            .await
            .unwrap()
            .exposure()
            .await
            .unwrap()
            .get()
            .await
            .unwrap();
        let ep = ExposureParameterization {
            min: Duration::from_secs_f64(exposure.min),
            max: Duration::from_secs_f64(exposure.max),
            step: Duration::from_secs_f64(exposure.step),
        };
        let exposure_stream = stream::iter(vec![MessageToClient::ExposureParameterization(ep)]);
        handle_websocket(socket.into(), stream::select(exposure_stream, to_message_to_client(capture))).await;
    }))
}

fn to_message_to_client<T, M>(
    state: impl Stream<Item = Result<T, TaskStatusError>> + std::marker::Unpin,
) -> impl Stream<Item = M> + Unpin
where
    M: ToWebsocketMessage + From<T>,
{
    state
        .map(|x| match x {
            Ok(x) => Some(x),
            Err(e) => {
                tracing::error!("Error streaming results: {:?}", e);
                None
            }
        })
        .filter(|option| futures::future::ready(option.is_some()))
        .map(|x| x.unwrap())
        .map(|x| x.into())
}

#[tracing::instrument(skip_all)]
async fn handle_websocket(
    socket: WebsocketHandler,
    capture_state: impl Stream<Item = MessageToClient> + Unpin,
) {
    socket.handle_websocket_stream(capture_state).await;
}
