use std::{ops::Deref, sync::Arc, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};

use serde::Deserialize;

use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use twinkle_api::{Count, CreateCountRequestParams, CreateCountResponse};
use twinkle_client::{
    notify::{ArcCounter, Notify},
    task::{self, Abortable, Status, Task},
};
use uuid::Uuid;

use crate::AppState;

pub fn routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/counts", post(create_async_task))
        .route("/counts/:id", get(subscribe_count))
        .route("/counts/:id", delete(delete_async_task))
}

#[tracing::instrument(skip_all)]
async fn subscribe_count(
    ws: WebSocketUpgrade,
    Query(params): Query<CreateCountRequestParams>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let id = uuid::Uuid::new_v4();

    let store = state.store.read().await;
    match store.runs.get(&id) {
        Some(task) => {
            let state = task.status().read().await
                .with_state(|state| {
                    let state = state.clone();
                    async move { state.subscribe().await }
                })
                .await;

            match state {
                Ok(sub) => Ok(ws.on_upgrade(move |socket| async move {
                    handle_websocket(socket, sub).await;
                })),
                Err(e) => {
                    tracing::error!("Unable to get task state: {:?}", e);
                    Err(StatusCode::NOT_FOUND)
                }
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[tracing::instrument(skip(socket))]
async fn handle_websocket(mut socket: WebSocket, mut stream: BroadcastStream<ArcCounter<Count>>) {
    loop {
        match stream.next().await {
            Some(Ok(item)) => {
                if let Err(e) = socket
                    .send(Message::Text(serde_json::to_string(item.deref()).unwrap()))
                    .await
                {
                    tracing::error!("Unable to send updated value: {:?}", e);
                    break;
                }
            }
            Some(Err(e)) => {
                tracing::error!("Error processing task status: {:?}", e);
                continue;
            }
            None => break,
        };
    }
}

#[tracing::instrument(skip(state))]
pub async fn create_async_task(State(state): State<AppState>) -> impl IntoResponse {
    let id = Uuid::new_v4();

    let mut store = state.store.write().await;

    store.runs.insert(
        id.clone(),
        task::spawn_with_state(
            Default::default(),
            |s: &Arc<twinkle_client::notify::Notify<Count>>| {
                let state = s.clone();
                async move {
                    loop {
                        let mut lock = state.write().await;
                        lock.count += 1;
                        dbg!(lock.count);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            },
        )
        .abort_on_drop(true),
    );

    Json(CreateCountResponse { id: id })
}

#[tracing::instrument(skip(state))]
pub async fn delete_async_task(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut store = state.store.write().await;

    if store.runs.remove(&id).is_some() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
