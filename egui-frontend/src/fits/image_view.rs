use std::sync::Arc;

use eframe::glow;
use egui::{mutex::Mutex, ProgressBar};
use ndarray::ArrayD;
use twinkle_api::analysis::Statistics;

use super::{FitsRender, FitsWidget, HoverInfo};

pub struct ImageView {
    state: State,
}

struct State {
    render: Arc<Mutex<FitsRender>>,
    progress: f32,
}

impl egui::Widget for &ImageView {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| {
            ui.add(FitsWidget::new(self.state.render.clone()));

            // Display pixel information if mouse is hovering over the image
            if let Some(hover_info) = self.state.render.lock().hover_info {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Pixel: ({}, {})  Intensity: {}",
                        hover_info.image_x, hover_info.image_y, hover_info.intensity
                    ));
                });
            }

            ui.add(ProgressBar::new(self.state.progress));
        })
        .response
    }
}

impl ImageView {
    pub fn new(gl: &glow::Context) -> ImageView {
        let state = State {
            render: Arc::new(Mutex::new(FitsRender::new(gl))),
            progress: 0.0,
        };
        ImageView { state }
    }

    pub fn set_progress(&mut self, progress: f32) {
        self.state.progress = progress;
    }

    pub fn set_image(&mut self, image: ArrayD<u16>, stats: Statistics) {
        let mut image_view = self.state.render.lock();
        image_view.set_fits(image);
        image_view.auto_stretch(&stats);
    }

    /// Get the current hover information (pixel coordinates and intensity)
    /// Returns None if the mouse is not hovering over the image
    pub fn get_hover_info(&self) -> Option<HoverInfo> {
        self.state.render.lock().hover_info
    }
}
