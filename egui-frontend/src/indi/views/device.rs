use crate::{fits::image_view::ImageView, Agent};
use eframe::glow;
use futures::executor::block_on;
use indi::client::active_device::ActiveDevice;
use indi::client::wait_fn;
use reqwest::IntoUrl;
use twinkle_client::notify;
use std::time::Duration;
use std::{collections::HashMap, ops::Deref};
use twinkle_client::task::Status::Running;
use twinkle_client::task::Task;

use super::tab;

pub struct Device {
    device: ActiveDevice,
    group: tab::TabView,
    parameters: HashMap<String, indi::Parameter>,
    blobs: HashMap<String, Agent<ImageView>>,
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

    pub async fn download_image(
        &mut self,
        name: String,
        gl: &glow::Context,
        url: impl IntoUrl + Clone + 'static,
    ) {
        let image_view_task = self.get_or_create_render(name, gl);
        let _ = wait_fn(&mut image_view_task.status().subscribe().await, Duration::from_millis(16), |asdf| {
            match asdf.deref() {
                twinkle_client::task::Status::Pending => Ok(notify::Status::Pending),
                Running(image_view) =>{
                    if let Err(e) = image_view.download_image(url.clone()) {
                        tracing::error!("Unable to download: {:?}", e);
                    }
    
                     Ok(notify::Status::Complete(()))
                },
                _ => Err(())
            }
            
        }).await;
    }

    #[tracing::instrument(skip_all)]
    fn get_or_create_render(
        &mut self,
        name: String,
        gl: &glow::Context,
    ) -> &mut Agent<ImageView> {
        self.blobs.entry(name).or_insert_with(|| ImageView::new(gl))
    }
}
