use egui::{Response, Ui, Widget};
use indi::client::active_device;

pub struct Parameter<'a> {
    parameter: &'a indi::Parameter,
    device: &'a active_device::ActiveDevice,
}

impl<'a> Parameter<'a> {
    pub fn new(parameter: &'a indi::Parameter, device: &'a active_device::ActiveDevice) -> Self {
        Parameter { parameter, device }
    }

    fn render_vectors(self, ui: &mut Ui) {
        match self.parameter {
            indi::Parameter::TextVector(text_vector) => {
                for (value_name, value) in &text_vector.values {
                    if let Some(label) = &value.label {
                        ui.label(label);
                    } else {
                        ui.label(value_name);
                    }
                    ui.label(format!("{}", &value.value));
                    ui.end_row();
                }
            }
            indi::Parameter::NumberVector(number_vector) => {
                for (value_name, value) in &number_vector.values {
                    if let Some(label) = &value.label {
                        ui.label(label);
                    } else {
                        ui.label(value_name);
                    }
                    ui.label(format!("{}", &value));
                    // error!("{:?}", &value.format);
                    ui.end_row();
                }
            }
            indi::Parameter::SwitchVector(switch_vector) => {
                for (value_name, value) in &switch_vector.values {
                    let label = if let Some(label) = &value.label {
                        label.clone()
                    } else {
                        value_name.clone()
                    };
                    let clicked_action = || {

                    };
                    if ui
                        .selectable_label(value.value == indi::SwitchState::On, label)
                        .clicked()
                    {
                        crate::task::spawn({
                            let active_device = self.device.clone();
                            let parameter_name = self.parameter.get_name().clone();
                            let value_name = value_name.clone();
                            async move {
                                active_device
                                    .change(&parameter_name, vec![(value_name.as_str(), true)])
                                    .await
                                    .ok();
                            }
                        });
                    }
                }
                ui.label("");
                ui.end_row();
            }
            indi::Parameter::LightVector(_light_vector) => {
                // for (value_name, value) in &light_vector.values {
                //     if let Some(label) = &value.label {
                //         ui.label(label);
                //     } else {
                //         ui.label(value_name);
                //     }
                //     ui.label(format!("{:?}", &value));
                //     ui.end_row();
                // }
            }
            indi::Parameter::BlobVector(_blob_vector) => {
                // for (value_name, value) in &blob_vector.values {
                //     if let Some(label) = &value.label {
                //         ui.label(label);
                //     } else {
                //         ui.label(value_name);
                //     }
                //     ui.label(format!("{:?}", &value));
                //     ui.end_row();
                // }
            }
        }
    }
}

impl<'a> Widget for Parameter<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        egui::Frame::default()
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .rounding(ui.visuals().widgets.noninteractive.rounding)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    egui::Grid::new(self.parameter.get_name())
                        .num_columns(2)
                        .show(ui, |ui| {
                            self.render_vectors(ui);
                        })
                        .response
                })
                .response
            })
            .response
    }
}
