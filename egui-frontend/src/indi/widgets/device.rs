use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, HashMap},
    ops::Deref,
    sync::Arc,
};

use egui::{Response, Ui, Widget};
use futures::executor::block_on;
use indi::client::Notify;
use itertools::Itertools;

pub struct Device<'a> {
    device: &'a indi::client::active_device::ActiveDevice,
    device_new: &'a mut HashMap<String, indi::Parameter>,
    group: &'a String,
}

impl<'a> Device<'a> {
    pub fn new(
        device: &'a indi::client::active_device::ActiveDevice,
        device_new: &'a mut HashMap<String, indi::Parameter>,
        group: &'a String,
    ) -> Self {
        Device { device, device_new, group }
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
                        if param.get_group().clone().is_some_and(|group| &group != self.group) {
                            return
                        }

                        match param {
                            indi::Parameter::NumberVector(vector) => {
                                if let Entry::Occupied(entry) =
                                    self.device_new.entry(param.get_name().clone())
                                {
                                    if !matches!(entry.get(), indi::Parameter::NumberVector(_)) {
                                        entry.remove_entry();
                                    }
                                }
                                let new_value = self
                                    .device_new
                                    .entry(param.get_name().clone())
                                    .or_insert_with(|| param.clone());
                                if let indi::Parameter::NumberVector(vector_new) = new_value {
                                    if let Some(label) = param.get_label() {
                                        ui.label(label);
                                    } else {
                                        ui.label(param.get_name());
                                    }
                                    ui.add(super::parameter::new(vector, self.device, vector_new));
                                    ui.end_row();
                                }
                            }
                            indi::Parameter::SwitchVector(vector) => {
                                if let Some(label) = param.get_label() {
                                    ui.label(label);
                                } else {
                                    ui.label(param.get_name());
                                }

                                ui.add(super::parameter::new(
                                    vector,
                                    self.device,
                                    &mut vector.clone(),
                                ));
                                ui.end_row();
                            }
                            indi::Parameter::TextVector(vector) => {
                                if let Entry::Occupied(entry) =
                                    self.device_new.entry(param.get_name().clone())
                                {
                                    if !matches!(entry.get(), indi::Parameter::TextVector(_)) {
                                        entry.remove_entry();
                                    }
                                }
                                let new_value = self
                                    .device_new
                                    .entry(param.get_name().clone())
                                    .or_insert_with(|| param.clone());
                                if let indi::Parameter::TextVector(vector_new) = new_value {
                                    if let Some(label) = param.get_label() {
                                        ui.label(label);
                                    } else {
                                        ui.label(param.get_name());
                                    }
                                    ui.add(super::parameter::new(vector, self.device, vector_new));
                                    ui.end_row();
                                }
                            }
                            _ => {}
                        }
                    });
                }
            })
            .response
    }
}
