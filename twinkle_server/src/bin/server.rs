use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade}, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};

use indi::client::{AsyncClientConnection, AsyncReadConnection, AsyncWriteConnection};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tower_http::services::ServeDir;
use tracing::error;

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

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/indi", get(create_connection))
        .fallback_service(ServeDir::new("assets"));

    // run our app with hyper
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct IndiConnectionParams {
    server_addr: String,
}

async fn create_connection(ws: WebSocketUpgrade, Query(params): Query<IndiConnectionParams>) -> Result<impl IntoResponse, StatusCode> {
    Ok(ws.on_upgrade(move |socket| handle_indi_connection(socket, params.server_addr)))
}

#[tracing::instrument(level = "info", skip(socket))]
async fn handle_indi_connection(socket: WebSocket, server_addr: String) {
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
