#![warn(clippy::all, rust_2018_idioms)]

mod app;

use std::ops::Deref;

pub use app::App;
use derive_more::{Deref, DerefMut, From};
use futures::executor::block_on;
use twinkle_client::task::{AsyncTask, Status, Task};

pub mod fits;
pub mod indi;
pub mod counts;
pub mod flats;
pub mod settings;
pub mod new_agent;

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

#[derive(From, Deref, DerefMut)]
pub struct Agent<S: std::marker::Sync>(AsyncTask<(), S>);

trait Widget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response;
}

impl<S: Send + Sync + 'static> egui::Widget for &Agent<S>
where
    for<'a> &'a S: crate::Widget,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let status = block_on(self.status().read());
        if let Status::Running(state) = status.deref() {
            state.ui(ui)
        } else {
            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
        }
    }
}

impl<S: Send + Sync + 'static> egui::Widget for &mut Agent<S>
where
    for<'a> &'a S: crate::Widget,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let status = block_on(self.status().read());
        if let Status::Running(state) = status.deref() {
            state.ui(ui)
        } else {
            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
        }
    }
}
