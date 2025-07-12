use eframe::glow;
use std::collections::HashMap;

use crate::{fits::image_view::ImageView, indi::agent::DeviceEntry};

pub struct ImageDevice {
    blobs: HashMap<String, ImageView>,
}

impl DeviceEntry for ImageDevice {
    fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        egui::Widget::ui(self, ui)
    }

    #[tracing::instrument(skip_all)]
    fn get_or_create_render(
        &mut self,
        name: String,
        gl: &glow::Context,
    ) -> &mut ImageView {
        self.blobs.entry(name).or_insert_with(|| ImageView::new(gl))
    }
}

impl egui::Widget for &mut ImageDevice {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        return ui
            .vertical(|ui| {
                for blob in self.blobs.values() {
                    ui.add(blob);
                }
            })
            .response;
    }
}

impl ImageDevice {
    pub fn new() -> Self {
        Self {
            blobs: Default::default(),
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn get_or_create_render(&mut self, name: String, gl: &glow::Context) -> &mut ImageView {
        self.blobs.entry(name).or_insert_with(|| ImageView::new(gl))
    }
}
