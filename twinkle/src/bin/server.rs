extern crate actix_web;
use std::{env, net::TcpStream};

use actix::prelude::*;
use actix_web::{HttpRequest, web, HttpResponse, Error, HttpServer, middleware, App, get};
use actix_web_actors::ws;
use futures_util::stream::once;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio_stream::{StreamExt, wrappers::{BroadcastStream, errors::BroadcastStreamRecvError}};

#[derive(Message)]
#[rtype(result = "()")]
struct Ping;

struct MyWs {
    recv: Receiver<Vec<String>>
}

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let stream = BroadcastStream::new(self.recv);
        Self::add_stream(stream, ctx);

    }
}
/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        println!("handle!");
    }
}

/// Handler for ws::Message message
impl StreamHandler<Result<Vec<String>, BroadcastStreamRecvError>> for MyWs {
    fn handle(&mut self, msg: Result<Vec<String>, BroadcastStreamRecvError>, ctx: &mut Self::Context) {
        println!("handle!");
    }
}

async fn index(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let sender = req.app_data::<Sender<Vec<String>>>().unwrap();
    let resp = ws::start(MyWs {recv: sender.subscribe() }, &req, stream);
    
    println!("{:?}", resp);
    resp
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=debug");

    let args: Vec<String> = env::args().collect();
    let connection = TcpStream::connect(&args[1]).unwrap();
    let client = indi::client::new(connection, None, None).unwrap();

    let devices = client.get_devices();
    let mut sub = devices.subscribe().unwrap();

    let (tx, _rx) = tokio::sync::broadcast::channel::<Vec<String>>(1024);

    let tx_c = tx.clone();
    tokio::spawn(async move {
        while let Some(Ok(update)) = sub.next().await  {
            let device_names: Vec<String> = update.keys().map(|x| x.into() ).collect();

            tx_c.send(device_names).unwrap();
            // {"type": "DeviceNames", value: ["device 1", "device 2"]}
        }
    });


    env_logger::init();

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(tx.clone())
            // Register HTTP request handlers
            .route("/ws/", web::get().to(index))
            .service(list_tweet)
            .service(get_tweet)
    })
    .bind("0.0.0.0:9090")?
    .run()
    .await
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Response<T> {
    pub results: Vec<T>,
}

pub type Tweets = Response<Tweet>;

#[derive(Debug, Deserialize, Serialize)]
pub struct Tweet {
    pub id: u32,
    pub message: String,
}

impl Tweet {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            id: rand::random::<u32>(),
            message: message.into(),
        }
    }

    pub fn foo(&self) {}
}

#[get("/tweets")]
pub async fn list_tweet() -> HttpResponse {
    let tweets = Tweets {
        results: vec![Tweet::new("First tweet!"), Tweet::new("last tweet")],
    };
    let resp = HttpResponse::Ok()
        .content_type("application/json")
        // .append_header(("access-control-allow-origin",  "*"))
        .json(tweets);

    dbg!(resp.body());
    resp
}

#[get("/tweets/{id}")]
pub async fn get_tweet(id: web::Path<u32>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        // .append_header(("access-control-allow-origin",  "*"))
        .json(Tweet::new(format!("A tweet with id: {}", id)))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TweetRequest {
    pub message: Option<String>,
}

impl TweetRequest {
    pub fn to_tweet(&self) -> Option<Tweet> {
        match &self.message {
            Some(message) => Some(Tweet::new(message.to_string())),
            None => None,
        }
    }
}
