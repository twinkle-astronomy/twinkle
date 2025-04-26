use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, HashMap},
    ops::Deref,
    sync::Arc,
};

use egui::{Color32, Response, RichText, Ui, Widget};
use futures::executor::block_on;
use indi::{client::Notify, Parameter};
use itertools::Itertools;
use twinkle_client::notify::NotifyMutexGuardRead;

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
        Device {
            device,
            device_new,
            group,
        }
    }
    fn show_parameter(&mut self, ui: &mut Ui, parameter: NotifyMutexGuardRead<'_, Parameter>) {
        let param = parameter.as_ref();
        if param
            .get_group()
            .clone()
            .is_some_and(|group| &group != self.group)
        {
            if let None = param.get_group() {
                ui.label(format!("{}: No group", param.get_name()));
                ui.end_row();
            }

            return;
        }
        match param {
            indi::Parameter::NumberVector(_) | indi::Parameter::TextVector(_) | indi::Parameter::SwitchVector(_) => {
                match param.get_state() {
                    indi::PropertyState::Idle => ui.label(RichText::new("•").weak()),
                    indi::PropertyState::Ok => ui.label(RichText::new("•").strong()),
                    indi::PropertyState::Busy => ui.spinner(),
                    indi::PropertyState::Alert => ui.label(RichText::new("!").strong().color(Color32::RED)),
                };
                if ui.label(param.get_label_display()).hovered() {
                    egui::show_tooltip(ui.ctx(), ui.layer_id(),egui::Id::new(param.get_label_display()), |ui| {
                        ui.label(format!("{:#?}", param));
                    });
                }
            }
            _ => {}
        }
        match param {
            indi::Parameter::NumberVector(vector) => {
                if let Entry::Occupied(entry) =
                    self.device_new.entry(param.get_name().clone())
                {
                    if !matches!(entry.get(), indi::Parameter::NumberVector(_))
                    {
                        entry.remove_entry();
                    }
                }
                let new_value = self
                    .device_new
                    .entry(param.get_name().clone())
                    .or_insert_with(|| param.clone());
                if let indi::Parameter::NumberVector(vector_new) = new_value {
                    ui.add(super::parameter::new(
                        vector,
                        self.device,
                        vector_new,
                    ));
                    ui.end_row();
                }
            }
            indi::Parameter::SwitchVector(vector) => {
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
                    ui.add(super::parameter::new(
                        vector,
                        self.device,
                        vector_new,
                    ));
                    ui.end_row();
                }
            }
            _ => {}
        }
    }
}

impl<'a> Widget for Device<'a> {
   
    fn ui(mut self, ui: &mut Ui) -> Response {
        let params: Vec<(String, Arc<Notify<indi::Parameter>>)> = block_on(async {
            self.device
                .deref()
                .read()
                .await
                .get_parameters()
                .iter()
                .sorted_by(|l, r| l.0.partial_cmp(r.0).unwrap_or(Ordering::Equal))
                .map(|(k, p)| (k.clone(), p.clone()))
                .collect()
        });
        ui.vertical(|ui| {
            egui::Grid::new("device")
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    for (_, param) in params {
                        let parameter = block_on(async {param.read().await});
                        self.show_parameter(ui, parameter);
                    }
                });
        })
        .response
    }
}
