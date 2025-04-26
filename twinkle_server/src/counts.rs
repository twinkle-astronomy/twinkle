use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
    sync::Arc,
    time::Duration,
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, Stream};

use tokio_stream::StreamExt;
use twinkle_api::{Count, CreateCountResponse};
use twinkle_client::{
    notify::ArcCounter,
    task::{self, AsyncTask, Task},
    OnDropFutureExt,
};
use uuid::Uuid;

use crate::AppState;

pub fn routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/counts", post(create_async_task))
        .route("/counts/:id", delete(delete_async_task))
        .route("/counts", get(subscribe_counts))
        .route("/counts/:id", get(subscribe_count))
}

#[tracing::instrument(skip_all)]
async fn subscribe_counts(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.store.read().await;
    let sub = store.runs.subscribe().await;
    Ok(ws.on_upgrade(move |socket| async move {
        handle_websocket_counts(socket, sub).await;
    }))
}

// #[tracing::instrument(skip(socket))]
async fn handle_websocket_counts(
    mut socket: WebSocket,
    mut stream: impl Stream<
            Item = Result<
                ArcCounter<
                    HashMap<Uuid, AsyncTask<(), Arc<twinkle_client::notify::Notify<Count>>>>,
                >,
                BroadcastStreamRecvError,
            >,
        > + Unpin,
) {
    loop {
        match stream.next().await {
            Some(Ok(item)) => {
                let ids: HashSet<Uuid> = item.keys().map(Clone::clone).collect();

                if let Err(e) = socket
                    .send(Message::Text(serde_json::to_string(&ids).unwrap()))
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

#[tracing::instrument(skip_all)]
async fn subscribe_count(
    ws: WebSocketUpgrade,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.store.read().await;
    let runs = store.runs.read().await;
    match runs.get(&id) {
        Some(task) => {
            let state = task
                .status()
                .read()
                .await
                .with_state(move |state| {
                    let state = state.clone();
                    async move { state.subscribe().await }
                })
                .await;

            match state {
                Ok(sub) => Ok(ws.on_upgrade(move |socket| {
                    async move {
                        handle_websocket_count(socket, sub).await;
                    }
                    .on_drop(|| tracing::info!("Dropped handle_websocket_count"))
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

#[tracing::instrument(skip_all)]
async fn handle_websocket_count(
    mut socket: WebSocket,
    mut stream: impl Stream<Item = Result<ArcCounter<Count>, BroadcastStreamRecvError>> + Unpin,
) {
    loop {
        match stream.next().await {
            Some(Ok(item)) => {
                let send_result = tokio::time::timeout(Duration::from_millis(1000), async {
                    socket
                        .send(Message::Text(serde_json::to_string(item.deref()).unwrap()))
                        .await
                })
                .await;

                match send_result {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        tracing::error!("Unable to send updated value: {:?}", e);
                        break;
                    }
                    Err(e) => {
                        tracing::error!("Timeout sending updated value: {:?}", e);
                        break;
                    }
                }
            }
            Some(Err(e)) => {
                tracing::error!("Error processing task status: {:?}", e);
                continue;
            }
            None => break,
        };
    }
    tracing::info!("Done with handle_websocket_count loop");
}

#[tracing::instrument(skip(state))]
pub async fn create_async_task(State(state): State<AppState>) -> impl IntoResponse {
    let id = Uuid::new_v4();

    let store = state.store.write().await;

    store.runs.write().await.insert(
        id.clone(),
        task::spawn_with_state(
            Default::default(),
            |s: &Arc<twinkle_client::notify::Notify<Count>>| {
                let state = s.clone();
                async move {
                    loop {
                        let mut lock = state.write().await;
                        lock.count += 1;
                        if lock.count < 100 {
                            tracing::info!("{}: {}", &id, lock.count);
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            },
        ),
    );

    Json(CreateCountResponse { id: id })
}

#[tracing::instrument(skip(state))]
pub async fn delete_async_task(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let store = state.store.write().await;

    if store.runs.write().await.remove(&id).is_some() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
