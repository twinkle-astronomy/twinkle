use std::ops::Deref;
use std::sync::Arc;
use axum::http::StatusCode;
use axum::{
    extract::{
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures::{StreamExt, TryStreamExt};

use twinkle_api::Settings;
use twinkle_client::notify::Notify;

use crate::websocket_handler::WebsocketHandler;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(get_settings))
        .route("/settings", post(set_settings))
}

#[tracing::instrument(skip(state))]
pub async fn set_settings(
    State(state): State<AppState>,
    Json(settings): Json<Settings>,
) -> impl IntoResponse {
    let mut store = state.store.write().await;
    if let Err(e) = store.save_settings(&settings).await {
        tracing::error!("Unable to save settings: {:?}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    *store.settings.write().await = settings;
    StatusCode::OK
}

#[tracing::instrument(skip_all)]
async fn get_settings(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.store.read().await;
    let settings = store.settings.clone();
    Ok(ws.on_upgrade(move |socket| async move {
        handle_websocket(socket.into(), settings).await;
    }))
}

#[tracing::instrument(skip_all)]
async fn handle_websocket(socket: WebsocketHandler, settings: Arc<Notify<Settings>>) {
    let sub = settings
        .subscribe()
        .await
        .map_ok(|item| {
            axum::extract::ws::Message::Text(serde_json::to_string(item.deref()).unwrap())
        })
        .take_while(|item| {
            if let Err(e) = &item {
                tracing::error!("Error streaming settings: {:?}", e);
            }
            futures::future::ready(item.is_ok())
        })
        .filter_map(|item| futures::future::ready(item.ok()));
    socket.handle_websocket_stream(sub).await;
}
