use std::cmp::Ordering;

use egui::{Response, Ui, Widget};
use indi::{
    client::active_device,
    serialization::{OneNumber, OneText},
};
use itertools::Itertools;

pub struct ParameterWidget<'a, T> {
    parameter: &'a T,
    device: &'a active_device::ActiveDevice,
    param_new: &'a mut T,
}

pub fn new<'a, T>(
    parameter: &'a T,
    device: &'a active_device::ActiveDevice,
    param_new: &'a mut T,
) -> ParameterWidget<'a, T> {
    ParameterWidget {
        parameter,
        device,
        param_new,
    }
}

impl<'a> ParameterWidget<'a, indi::NumberVector> {
    fn render_parameters(&mut self, ui: &mut Ui) {
        match self.parameter.perm {
            indi::PropertyPerm::RO => {
                for (value_name, value) in &self.parameter.values {
                    if let Some(label) = &value.label {
                        ui.label(label);
                    } else {
                        ui.label(value_name);
                    }
                    ui.label(format!("{}", &value));
                    ui.end_row();
                }
            }
            indi::PropertyPerm::RW => {
                for (value_name, value) in &self.parameter.values {
                    let new_value = self
                        .param_new
                        .values
                        .entry(value_name.to_string())
                        .or_insert_with(|| value.clone());
                    if let Some(label) = &value.label {
                        ui.label(label);
                    } else {
                        ui.label(value_name);
                    }
                    ui.horizontal(|ui| {
                        ui.label(format!("{}", &value));
                        ui.add(egui::DragValue::new(&mut new_value.value.hour));
                        if let Some(mut minute) = new_value.value.minute {
                            ui.label(":");
                            ui.add(egui::DragValue::new(&mut minute));
                        }
                        if let Some(mut second) = new_value.value.second {
                            ui.label(":");
                            ui.add(egui::DragValue::new(&mut second));
                        }
                    });
                    ui.end_row();
                }
            }
            indi::PropertyPerm::WO => {}
        }
    }

    fn render_set_button(&mut self, ui: &mut Ui) -> Response {
        if self.parameter.perm == indi::PropertyPerm::RW {
            let response = ui.button("set");
            if response.clicked() {
                crate::task::spawn((), |_| {
                    let active_device = self.device.clone();
                    let parameter_name = self.parameter.name.clone();
                    let values: std::vec::Vec<OneNumber> = self
                        .param_new
                        .values
                        .iter()
                        .map(|(value_name, value)| OneNumber {
                            name: value_name.clone(),
                            value: value.value,
                        })
                        .collect();

                    async move {
                        active_device.change(&parameter_name, values).await.ok();
                    }
                });
            }
            response
        } else {
            // Return an empty response if button isn't shown
            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
        }
    }
}

impl<'a> Widget for ParameterWidget<'a, indi::NumberVector> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        egui::Frame::default()
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            // .rounding(ui.visuals().widgets.noninteractive.rounding)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    egui::Grid::new(&self.parameter.name)
                        .num_columns(4)
                        .show(ui, |ui| {
                            self.render_parameters(ui);
                        })
                })
                .response
            })
            .response
            | self.render_set_button(ui)
    }
}

impl<'a> Widget for ParameterWidget<'a, indi::SwitchVector> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            for (value_name, value) in &self.parameter.values {
                
                let label = if let Some(label) = &value.label {
                    label.clone()
                } else {
                    value_name.clone()
                };
                let selectable_label = ui.selectable_label(value.value == indi::SwitchState::On, label);
                if selectable_label
                    .clicked()
                {
                    crate::task::spawn((), |_| {
                        let active_device = self.device.clone();
                        let parameter_name = self.parameter.name.clone();
                        let value_name = value_name.clone();
                        let value = value.value == indi::SwitchState::Off;
                        async move {
                            active_device
                                .change(&parameter_name, vec![(value_name.as_str(), value)])
                                .await
                                .ok();
                        }
                    });
                }
            }
        })
        .response
    }
}

impl<'a> ParameterWidget<'a, indi::TextVector> {
    fn render_parameters(&mut self, ui: &mut Ui) {
        match self.parameter.perm {
            indi::PropertyPerm::RO => {
                for (value_name, value) in &self.parameter.values {
                    if let Some(label) = &value.label {
                        ui.label(label);
                    } else {
                        ui.label(value_name);
                    }
                    ui.label(format!("{}", &value.value));
                    ui.end_row();
                }
            }
            indi::PropertyPerm::RW => {
                for (value_name, value) in self
                    .parameter
                    .values
                    .iter()
                    .sorted_by(|l, r| l.0.partial_cmp(r.0).unwrap_or(Ordering::Equal))
                {
                    let new_value = self
                        .param_new
                        .values
                        .entry(value_name.to_string())
                        .or_insert_with(|| value.clone());

                    if let Some(label) = &value.label {
                        ui.label(label);
                    } else {
                        ui.label(value_name);
                    }
                    ui.horizontal(|ui| {
                        ui.label(&value.value);
                        ui.text_edit_singleline(&mut new_value.value);
                    });
                    ui.end_row();
                }
            }
            indi::PropertyPerm::WO => {}
        }
    }

    fn render_set_button(&mut self, ui: &mut Ui) -> Response {
        if self.parameter.perm == indi::PropertyPerm::RW {
            let response = ui.button("set");
            if response.clicked() {
                crate::task::spawn((), |_| {
                    let active_device = self.device.clone();
                    let parameter_name = self.parameter.name.clone();
                    let values: std::vec::Vec<OneText> = self 
                        .param_new
                        .values
                        .iter()
                        .map(|(value_name, value)| OneText {
                            name: value_name.clone(),
                            value: value.value.clone(),
                        })
                        .collect();

                    async move {
                        active_device.change(&parameter_name, values).await.ok();
                    }
                });
            }
            response
        } else {
            // Return an empty response if button isn't shown
            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
        }
    }
}

impl<'a> Widget for ParameterWidget<'a, indi::TextVector> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        egui::Frame::default()
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            // .rounding(ui.visuals().widgets.noninteractive.rounding)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    egui::Grid::new(&self.parameter.name)
                        .num_columns(2)
                        .show(ui, |ui| {
                            self.render_parameters(ui);
                        })
                })
                .response
            })
            .response
            | self.render_set_button(ui)
    }
}
