extern crate actix_web;
use std::{env, net::TcpStream, sync::Arc, collections::HashMap};

use actix::prelude::*;
use actix_web::{HttpRequest, web::{self, Data}, HttpResponse, Error, HttpServer, middleware, App, get};

use indi::client::{notify::Notify, device::Device};
use serde::{Deserialize, Serialize};
use tokio::{sync::{broadcast::{Receiver, Sender}, mpsc}, task::spawn_local};
use tokio_stream::{StreamExt, wrappers::{BroadcastStream, errors::BroadcastStreamRecvError}};

/// Handshake and start WebSocket handler with heartbeats.
async fn chat_ws(
    req: HttpRequest,
    stream: web::Payload,
    devices: web::Data<Arc<Notify<HashMap<String, Arc<Notify<Device>>>>>>,
) -> Result<HttpResponse, Error> {
    let (res, mut session, _msg_stream) = actix_ws::handle(&req, stream)?;

    let mut devices = devices.subscribe().unwrap();

    // spawn websocket handler (and don't await it) so that the response is returned immediately
    tokio::spawn(async move {
        while let Some(Ok(devices)) = devices.next().await {
            let device_names: Vec<String> = devices.keys().map(|x| x.into() ).collect();
            if let Err(e) = session.text(format!("{:?}", device_names)).await {
                dbg!(e);
                break
            }
        }
    
        // attempt to close connection gracefully
        let _ = session.close(None).await;
    });

    Ok(res)
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=debug");

    let args: Vec<String> = env::args().collect();
    let connection = TcpStream::connect(&args[1]).unwrap();
    let client = indi::client::new(connection, None, None).unwrap();

    let devices = client.get_devices();

    env_logger::init();

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(Data::new(devices.clone()))
            // Register HTTP request handlers
            .service(web::resource("/ws").route(web::get().to(chat_ws)))
    })
    .bind("0.0.0.0:9090")?
    .run()
    .await
}
