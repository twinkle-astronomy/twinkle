use futures::executor::block_on;
use indi::client::active_device::ActiveDevice;
use std::collections::HashMap;

use crate::{fits::image_view::ImageView, indi::agent::DeviceEntry};

use super::tab;

pub struct Device {
    device: ActiveDevice,
    group: tab::TabView,
    parameters: HashMap<String, indi::Parameter>,
    blobs: HashMap<String, ImageView>,
}

impl DeviceEntry for Device {
    fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        egui::Widget::ui(self, ui)
    }
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
}
