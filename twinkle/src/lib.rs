/// We derive Deserialize/Serialize so we can persist app state on shutdown.
mod backend;
use backend::*;

use fits_inspect::egui::FitsWidget;

use tracing::{event, Level};

use indi::Parameter;

pub struct TwinkleApp {
    backend: Backend,

    address: String,

    selected_device: Option<String>,
    selected_group: Option<String>,
    fits_viewer: Option<FitsWidget>,
}

impl Default for TwinkleApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            address: "localhost:7624".to_owned(),
            backend: Default::default(),

            selected_device: None,
            selected_group: None,
            fits_viewer: None,
        }
    }
}

impl TwinkleApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut newed: TwinkleApp = Default::default();
        newed.fits_viewer = FitsWidget::new(cc);
        newed
    }
}

impl eframe::App for TwinkleApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self {
            address,
            backend,
            selected_device,
            selected_group,
            fits_viewer,
        } = self;

        let client_lock = backend.get_client(); //.lock().unwrap();
        let client = client_lock.lock().unwrap();
        let devices = client.get_devices();

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Indi: ");
                ui.text_edit_singleline(address);
            });

            match backend.get_status() {
                ConnectionStatus::Disconnected => {
                    if ui.button("Connect").clicked() {
                        if let Err(e) = backend.connect(ctx.clone(), address.to_string()) {
                            event!(Level::ERROR, "Connection error: {:?}", e);
                        }
                    }
                }
                ConnectionStatus::Connecting => {
                    ui.label(format!("Connecting to {}", address));
                }
                ConnectionStatus::Initializing => {
                    ui.label(format!("Initializing connection"));
                }
                ConnectionStatus::Connected => {
                    if ui.button("Disconnect").clicked() {
                        if let Err(e) = backend.disconnect() {
                            event!(Level::ERROR, "Disconnection error: {:?}", e);
                        }
                    }
                }
            }

            ui.separator();

            {
                for (name, device) in devices {
                    if ui
                        .selectable_value(&mut Some(name), selected_device.as_ref(), name)
                        .clicked()
                    {
                        *selected_device = Some(name.to_string());
                        if device.parameter_groups().len() > 0 {
                            *selected_group = device.parameter_groups()[0].clone();
                        } else {
                            *selected_group = None;
                        }
                    }
                }
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    egui::warn_if_debug_build(ui);
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(fits_viewer) = fits_viewer {
                fits_viewer.update(ctx, _frame);
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show_viewport(ui, |ui, _viewport| {
                    if let Some(device_name) = selected_device {
                        if let Some(device) = devices.get(device_name) {
                            ui.heading(device_name.clone());
                            ui.separator();
                            ui.horizontal(|ui| {
                                for group in device.parameter_groups() {
                                    if ui
                                        .add(egui::SelectableLabel::new(
                                            group == selected_group,
                                            group.clone().unwrap_or("".to_string()),
                                        ))
                                        .clicked()
                                    {
                                        *selected_group = group.clone();
                                    }
                                }
                            });
                            ui.separator();
                            for (name, param) in device
                                .get_parameters()
                                .iter()
                                .filter(|(_, p)| p.get_group() == selected_group)
                            {
                                ui.label(name);
                                ui.separator();

                                match param {
                                    Parameter::TextVector(tv) => {
                                        egui::Grid::new(format!("{}", name)).num_columns(2).show(
                                            ui,
                                            |ui| {
                                                for (text_name, text_value) in &tv.values {
                                                    ui.label(text_name.clone());
                                                    ui.label(text_value.value.clone());
                                                    ui.end_row();
                                                }
                                            },
                                        );
                                    }
                                    Parameter::NumberVector(nv) => {
                                        egui::Grid::new(format!("{}", name)).num_columns(2).show(
                                            ui,
                                            |ui| {
                                                for (number_name, number_value) in &nv.values {
                                                    ui.label(number_name.clone());
                                                    ui.label(format!("{}", number_value.value));
                                                    ui.end_row();
                                                }
                                            },
                                        );
                                    }
                                    Parameter::SwitchVector(sv) => {
                                        ui.horizontal(|ui| {
                                            for (button_name, button_value) in &sv.values {
                                                if ui
                                                    .add(egui::SelectableLabel::new(
                                                        button_value.value == indi::SwitchState::On,
                                                        button_name.clone(),
                                                    ))
                                                    .clicked()
                                                {
                                                    backend.write(
                                                        &indi::Command::NewSwitchVector(
                                                            indi::NewSwitchVector {
                                                                device: device_name.to_string(),
                                                                name: name.to_string(),
                                                                timestamp: None,
                                                                switches: vec![indi::OneSwitch {
                                                                    name: button_name.to_string(),
                                                                    value: indi::SwitchState::On,
                                                                }],
                                                            },
                                                        ),
                                                    ).unwrap_or_else(|e| {dbg!(e);});
                                                }
                                            }
                                        });
                                    }
                                    Parameter::BlobVector(bv) => {
                                        for (name, _blob) in &bv.values {
                                            ui.label(format!("BLOB {}", name));
                                            if ui.button("Images").clicked() {
                                                let enable_blob = indi::EnableBlob {
                                                    device: device_name.clone(),
                                                    name: None,
                                                    enabled: indi::BlobEnable::Also,
                                                };
                                                backend.write(&indi::Command::EnableBlob(
                                                    enable_blob,
                                                )).unwrap_or_else(|e| {dbg!(e);});
                                            }
                                        }
                                    }
                                    _ => {}
                                }

                                ui.end_row();
                            }
                        }
                    }
                });
        });
        // self.backend.tick();
    }
}
