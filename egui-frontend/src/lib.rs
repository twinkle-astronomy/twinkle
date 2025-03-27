#![warn(clippy::all, rust_2018_idioms)]

mod app;

use std::ops::Deref;

pub use app::App;
use derive_more::{Deref, DerefMut, From};
use futures::executor::block_on;
use twinkle_client::task::{AsyncTask, Status, Task};

pub mod fits;
pub mod indi;

#[derive(From, Deref, DerefMut)]
pub struct Agent<T, S>(AsyncTask<T, S>);

trait Widget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response;
}

impl<S: Send + Sync + 'static> egui::Widget for &Agent<(), S>
where
    for<'a> &'a S: crate::Widget,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let status = block_on(self.status().lock());
        if let Status::Running(state) = status.deref() {
            state.ui(ui)
        } else {
            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
        }
    }
}

impl<S: Send + Sync + 'static> egui::Widget for &mut Agent<(), S>
where
    for<'a> &'a S: crate::Widget,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let status = block_on(self.status().lock());
        if let Status::Running(state) = status.deref() {
            state.ui(ui)
        } else {
            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
        }
    }
}
