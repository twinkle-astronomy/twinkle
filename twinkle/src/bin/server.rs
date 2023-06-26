extern crate actix_web;
use std::{collections::HashMap, env, net::TcpStream, sync::Arc};

use actix_web::{
    middleware,
    web::{self, Data},
    App, Error, HttpRequest, HttpResponse, HttpServer,
};

use client::{notify::Notify, StreamExt as _};
use indi::client::device::Device;
use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, StreamExt};

/// Handshake and start WebSocket handler with heartbeats.
async fn chat_ws(
    req: HttpRequest,
    stream: web::Payload,
    devices: web::Data<Arc<Notify<HashMap<String, Arc<Notify<Device>>>>>>,
) -> Result<HttpResponse, Error> {
    let (res, mut session, _msg_stream) = actix_ws::handle(&req, stream)?;

    let devices = devices.subscribe().unwrap();

    // spawn websocket handler (and don't await it) so that the response is returned immediately
    tokio::spawn(async move {
        let mut device_names = devices
            .map(|device| -> Result<Vec<String>, BroadcastStreamRecvError> {
                Ok(device?.keys().map(String::clone).collect::<Vec<String>>())
            })
            .changes();
        while let Some(Ok(device_names)) = device_names.next().await {
            if let Err(e) = session.text(format!("{:?}", device_names)).await {
                dbg!(e);
                break;
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
