#![warn(clippy::all, rust_2018_idioms)]

mod app;

pub use app::App;
pub mod agent;

pub mod capture;
pub mod fits;
pub mod flats;
pub mod indi;
pub mod settings;

#[cfg(debug_assertions)]
fn get_websocket_base() -> String {
    format!("ws://localhost:4000/")
}

#[cfg(not(debug_assertions))]
fn get_websocket_base() -> String {
    format!("/")
}

#[cfg(debug_assertions)]
fn get_http_base() -> String {
    format!("http://localhost:4000/")
}

#[cfg(not(debug_assertions))]
fn get_http_base() -> String {
    web_sys::window()
        .expect("No global window exists")
        .location()
        .href()
        .expect("No location exists")
}
