extern crate actix_web;

use actix_web::{get, middleware, web, App, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use std::{env, io};

#[actix_rt::main]
async fn main() -> io::Result<()> {
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=debug");
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            // Register HTTP request handlers
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
