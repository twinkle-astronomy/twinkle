use std::{collections::HashMap, sync::Arc};

use axum::{
    body::Body,
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{header, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};

use axum_extra::TypedHeader;
use indi::{
    client::{AsyncClientConnection, AsyncReadConnection, AsyncWriteConnection},
    serialization::{Blob, OneBlob, SetBlobVector},
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpStream, sync::RwLock};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::error;
use twinkle_api::{analysis::Statistics, fits::FitsImage, indi::api::ImageResponse};
use urlencoding::encode;
use uuid::Uuid;

#[derive(Debug, Default, Clone)]
struct AppState {
    // Store device data by device name
    store: Arc<RwLock<StateData>>,
}

#[derive(Debug, Default)]
struct StateData {
    connections: HashMap<Uuid, Arc<RwLock<IndiConnectionData>>>,
}

#[derive(Debug, Default)]
struct IndiConnectionData {
    blobs: HashMap<String, Arc<indi::serialization::SetBlobVector>>,
}

// Requests
#[derive(Deserialize, Serialize)]
struct CreateConnection {
    addr: String,
}

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .init();
    let state = AppState::default();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(Any)
        .expose_headers(Any);

    // build our application with a route
    let app = Router::new()
        .route("/indi", get(create_connection))
        .route("/indi/blob/:id/:device/:parameter/:value", get(get_blob))
        .with_state(state)
        .layer(cors)
        .fallback_service(ServeDir::new("assets"));

    // run our app with hyper
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize, Debug)]
struct IndiConnectionParams {
    server_addr: String,
}

#[tracing::instrument(skip(ws, state))]
async fn create_connection(
    ws: WebSocketUpgrade,
    TypedHeader(host): TypedHeader<axum_extra::headers::Host>, // Use axum_extra::headers
    Query(params): Query<IndiConnectionParams>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    Ok(ws.on_upgrade(move |socket| async move {
        let id = uuid::Uuid::new_v4();
        let connection_data = Arc::new(RwLock::new(Default::default()));

        {
            let mut store = state.store.write().await;
            store.connections.insert(id, connection_data.clone());
        }

        handle_indi_connection(id, connection_data, socket, params.server_addr, host).await;
        {
            let mut store = state.store.write().await;
            store.connections.remove(&id);
        }
    }))
}

#[tracing::instrument(skip(socket))]
async fn handle_indi_connection(
    id: Uuid,
    connection_data: Arc<RwLock<IndiConnectionData>>,
    socket: WebSocket,
    server_addr: String,
    host: axum_extra::headers::Host,
) {
    let connection = match TcpStream::connect(server_addr).await {
        Ok(c) => c,
        Err(e) => {
            error!("Error: {:?}", e);
            socket.close().await.ok();
            return;
        }
    };
    let (mut indi_writer, mut indi_reader) = connection.to_indi();
    let (mut websocket_write, mut websocket_read) = socket.to_indi();

    let writer = tokio::spawn(async move {
        loop {
            let cmd = match websocket_read.read().await {
                Some(Ok(c)) => c,
                Some(Err(_)) | None => break,
            };
            if let Err(e) = indi_writer.write(cmd).await {
                error!("Error sending command to indi server: {:?}", e);
            }
        }
    });
    let reader = tokio::spawn(async move {
        loop {
            match indi_reader.read().await {
                Some(Ok(cmd)) => {
                    let cmd = match cmd {
                        indi::serialization::Command::SetBlobVector(sbv) => {
                            let cmd = indi::serialization::Command::SetBlobVector(SetBlobVector {
                                device: sbv.device.clone(),
                                name: sbv.name.clone(),
                                state: sbv.state.clone(),
                                timeout: sbv.timeout.clone(),
                                timestamp: sbv.timestamp.clone(),
                                message: sbv.message.clone(),
                                blobs: sbv
                                    .blobs
                                    .iter()
                                    .map(|x| OneBlob {
                                        name: x.name.clone(),
                                        size: x.size.clone(),
                                        enclen: x.enclen.clone(),
                                        format: "download".to_string(),
                                        value: Blob(
                                            format!(
                                                "http://{}/indi/blob/{}/{}/{}/{}",
                                                host,
                                                id,
                                                encode(&sbv.device),
                                                encode(&sbv.name),
                                                encode(&x.name)
                                            )
                                            .as_bytes()
                                            .to_vec(),
                                        ),
                                    })
                                    .collect(),
                            });
                            let image_id = format!("{}.{}", sbv.device, sbv.name);
                            let sbv = Arc::new(sbv);
                            connection_data
                                .write()
                                .await
                                .blobs
                                .entry(image_id)
                                .and_modify(|entry| {
                                    *entry = sbv.clone();
                                })
                                .or_insert(sbv);
                            cmd
                        }
                        cmd => cmd,
                    };
                    websocket_write.write(cmd).await.unwrap();
                }
                Some(Err(e)) => {
                    error!("Error reading from indi server: {:?}", &e);
                }
                None => break,
            }
        }
    });

    if let Err(e) = tokio::try_join!(reader, writer) {
        tracing::error!("Error: {:?}", e);
    }
}


#[tracing::instrument(skip(state))]
async fn get_blob(
    Path((id, device, parameter, value)): Path<(Uuid, String, String, String)>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let connection_data = match state.store.read().await.connections.get(&id) {
        Some(cd) => cd.clone(),
        None => {
            return Err(StatusCode::NOT_FOUND);
        }
    };
    let image_id = format!("{}.{}", &device, &parameter);
    let sbv = match connection_data.read().await.blobs.get(&image_id) {
        Some(sbv) => sbv.clone(),
        None => {
            return Err(StatusCode::NOT_FOUND);
        }
    };
    let blob = match sbv.blobs.iter().find(|x| x.name == value) {
        Some(blob) => blob,
        None => {
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let mut bytes = Vec::new();

    let image = FitsImage::new(&blob.value.0);
    let stats = Statistics::new(&image.read_image().unwrap().view());
    let image_data = ImageResponse {
        stats,
        image,
    };
    image_data.to_bytes(&mut bytes);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/msgpack")
        .body(Body::from(bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response)
}
