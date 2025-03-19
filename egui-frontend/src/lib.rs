#![warn(clippy::all, rust_2018_idioms)]

mod app;

use std::ops::Deref;

pub use app::App;
use futures::executor::block_on;
use task::{AsyncTask, Status, Task};

pub mod fits;
pub mod indi;
pub mod task;


trait Widget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response;
}

impl<T> egui::Widget for &Status<T> 
where
    for<'a> &'a T: Widget
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        if let Status::Running(state) = self {
            state.ui(ui)
        } else {
            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
        }
    }
}

impl<S: 'static> egui::Widget for &AsyncTask<(), S> 
where
    for<'a> &'a S: crate::Widget
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {

        let status = block_on(self.status().lock());
        ui.add(status.deref())
    }
}

impl<S: 'static> egui::Widget for &mut AsyncTask<(), S> 
where
    for<'a> &'a S: crate::Widget
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {

        let status = block_on(self.status().lock());
        ui.add(status.deref())
    }
}