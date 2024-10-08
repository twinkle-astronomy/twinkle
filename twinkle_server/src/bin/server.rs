use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade}, http::StatusCode, response::IntoResponse, routing::get, Router
};


use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use indi::client::{AsyncClientConnection, AsyncReadConnection, AsyncWriteConnection};

// Requests
#[derive(Deserialize, Serialize)]
struct CreateConnection {
    addr: String
}

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(create_connection));

    // run our app with hyper
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}


async fn create_connection(ws: WebSocketUpgrade) -> Result<impl IntoResponse, StatusCode>  {
    Ok(ws.on_upgrade(move |socket| handle_indi_connection(socket)))
}

async fn handle_indi_connection(socket: WebSocket) {
    let connection = match TcpStream::connect("indi:7624").await {
        Ok(c) => {
            c
        },
        Err(_) => {
            socket.close().await.ok();
            return
        }
    };
    let (mut indi_writer, mut indi_reader) = connection.to_indi();
    let (mut websocket_write, mut websocket_read ) = socket.to_indi();

    let writer = tokio::spawn(async move {
        loop {
            let cmd = match websocket_read.read().await {
                Some(Ok(c)) => c,
                Some(Err(_)) | None => break,
            };
            dbg!(&cmd);

            if let Err(e) = indi_writer.write(cmd).await {
                dbg!(e);
            }
        }
    });

    let reader = tokio::spawn(async move {
        loop {
            match indi_reader.read().await {
                Some(Ok(cmd)) => {
                    dbg!(&cmd);
                    websocket_write.write(cmd).await.unwrap();
                    
                },
                Some(Err(e)) => {
                    dbg!(&e);
                }
                None => break,
            }
        }
    });

    if let Err(e) = tokio::try_join!(reader, writer) {
        tracing::error!("Error: {:?}", e);
    }
}

