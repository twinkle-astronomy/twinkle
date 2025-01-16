use std::{cmp::Ordering, ops::Deref, sync::Arc};

use egui::{Response, Ui, Widget};
use futures::executor::block_on;
use indi::client::Notify;
use itertools::Itertools;

pub struct Device<'a> {
    device: &'a indi::client::active_device::ActiveDevice,
}

impl<'a> Device<'a> {
    pub fn new(device: &'a indi::client::active_device::ActiveDevice) -> Self {
        Device { device }
    }
}

impl<'a> Widget for Device<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let params: Vec<(String, Arc<Notify<indi::Parameter>>)> = block_on(async {
            self.device
                .deref()
                .lock()
                .await
                .get_parameters()
                .iter()
                .sorted_by(|l, r| l.0.partial_cmp(r.0).unwrap_or(Ordering::Equal))
                .map(|(k, p)| (k.clone(), p.clone()))
                .collect()
        });

        egui::Grid::new("device")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for (_, param) in params {
                    block_on(async {
                        let parameter = param.lock().await;
                        let param = parameter.as_ref();
                        if let Some(label) = param.get_label() {
                            ui.label(label);
                        } else {
                            ui.label(param.get_name());
                        }
                        ui.add(super::Parameter::new(param, self.device));
                        ui.end_row();
                    });
                }
            })
            .response
    }
}
