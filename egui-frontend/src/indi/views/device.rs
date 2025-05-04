// use crate::fits::image_view::ImageView;
use eframe::glow;
use futures::executor::block_on;
use indi::client::active_device::ActiveDevice;
use std::collections::HashMap;

use crate::fits::image_view::ImageView;

use super::tab;

pub struct Device {
    device: ActiveDevice,
    group: tab::TabView,
    parameters: HashMap<String, indi::Parameter>,
    blobs: HashMap<String, ImageView>,
}

impl egui::Widget for &mut Device {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let group = {
            let binding = block_on(self.device.read());
            let groups = binding.parameter_groups().iter();
            self.group.show(ui, groups)
        };
        ui.separator();
        if let Some(group) = group {
            ui.vertical(|ui| {
                ui.add(crate::indi::widgets::device::Device::new(
                    &self.device,
                    &mut self.parameters,
                    group,
                ));

                for blob in self.blobs.values() {
                    ui.add(blob);
                }
            })
            .response
        } else {
            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
        }
    }
}

impl Device {
    pub fn new(device: ActiveDevice) -> Self {
        Self {
            device,
            group: Default::default(),
            parameters: Default::default(),
            blobs: Default::default(),
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn get_or_create_render(
        &mut self,
        name: String,
        gl: &glow::Context,
    ) -> &mut ImageView {
        self.blobs.entry(name).or_insert_with(|| ImageView::new(gl))
    }
}
