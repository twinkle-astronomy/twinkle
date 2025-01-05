
use std::{collections::{HashMap, HashSet}, sync::Arc};

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade}, http::{Request, StatusCode}, response::IntoResponse, routing::get, Router
};

use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse};

use futures::{channel::oneshot, stream, SinkExt, Stream, StreamExt};
use indi::client::{device::Device, AsyncClientConnection, AsyncReadConnection, AsyncWriteConnection, MemoryDeviceStore, Notify};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpStream, sync::broadcast::error::RecvError};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tracing::{info, Span};
use std::fmt::Debug;

// Requests
#[derive(Deserialize, Serialize)]
struct CreateConnection {
    addr: String
}

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .init();

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/indi", get(create_connection))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(true))

        );

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

    let (mut indi_write, mut indi_read) = connection.to_indi();
    let (mut websocket_write, mut websocket_read ) = socket.split();
    let writer = async move {
        loop {
            let msg = match indi_read.read().await {
                Some(Ok(msg)) => msg,
                Some(Err(e)) => break Err(e),
                None => break Ok(()),
            };

            websocket_write.send(Message::Text(quick_xml::se::to_string(&msg).unwrap())).await.unwrap();
        }
    };

    let reader = async move {
        loop {
            let msg = match websocket_read.next().await {
                Some(Ok(msg)) => msg,
                Some(Err(e)) => break Err(e),
                None => break Ok(()),
            };

            if let Message::Text(msg) = msg {

                indi_write.write(
                    quick_xml::de::from_str(msg.as_str()).unwrap()
                ).await.unwrap();    
            }
                    
        }
    };
    tokio::select! {
        res = reader => {
            info!("Reader closed: {:?}", res);
        },
        res = writer => {
            info!("Writer closed: {:?}", res);
        }
    }
}
