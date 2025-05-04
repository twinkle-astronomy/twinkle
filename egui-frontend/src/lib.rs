#![warn(clippy::all, rust_2018_idioms)]

mod app;

pub use app::App;

pub mod fits;
pub mod flats;
pub mod indi;
pub mod settings;

pub mod sync_task;

#[cfg(debug_assertions)]
fn get_websocket_base() -> String {
    format!("ws://localhost:4000/")
}

#[cfg(not(debug_assertions))]
fn get_websocket_base() -> String {
    // format!("/indi?server_addr={}", encoded_value)
    format!("/")
}

#[cfg(debug_assertions)]
fn get_http_base() -> String {
    format!("http://localhost:4000/")
}

#[cfg(not(debug_assertions))]
fn get_http_base() -> String {
    // format!("/indi?server_addr={}", encoded_value)
    format!("/")
}
