use std::{sync::Arc, time::Duration};

use egui::Window;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite_wasm::Message;
use indi::telescope::settings::Settings;
use twinkle_client::{
    sleep,
    task::{spawn, Abortable, IsRunning},
};

use crate::{agent::{Agent, AgentLock, Widget}, get_http_base};

fn get_websocket_url() -> String {
    format!("{}settings", crate::get_websocket_base())
}

fn post_url() -> String {
    format!("{}settings", get_http_base())
}

pub struct SettingsManager {
    agent: Agent<SettingsWidget>,
}

impl SettingsManager {
pub fn new() -> Self {
        SettingsManager { agent: Default::default() }
    }


    pub fn windows(&mut self, ui: &mut egui::Ui) {
        if self.agent.running() {
            let mut open = true;
            Window::new("Settings")
                .open(&mut open)
                .resizable(true)
                .scroll([false, false])
                .show(ui.ctx(), |ui| ui.add(&mut self.agent));
            if !open {
                self.agent.abort();
            }
        }
    }
}
impl egui::Widget for &mut SettingsManager {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| match self.agent.running() {
            true => {
                if ui.selectable_label(true, "Settings").clicked() {
                    self.agent.abort();
                }
            }
            false => {
                if ui.selectable_label(false, "Settings").clicked() {
                    self.agent.spawn(ui.ctx().clone(), Default::default(), |state| SettingsWidget::task(state));
                }
            }
        })
        .response
    }
}

#[derive(Default)]
pub struct SettingsWidget {
    data: Option<SettingsData>,
}

impl SettingsWidget {

    async fn task(state: Arc<AgentLock<SettingsWidget>>) {
        loop {
            let websocket_url = get_websocket_url();
            let websocket = match tokio_tungstenite_wasm::connect(websocket_url).await {
                Ok(websocket) => websocket,
                Err(e) => {
                    tracing::error!("Failed to connect to {}: {:?}", get_websocket_url(), e);
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };
            let (mut w, mut r) = websocket.split();
            let (_tx, rx) = tokio::sync::oneshot::channel::<()>();
            let close_task = spawn((), move |_| async move {
                if let Err(_) = rx.await {
                    if let Err(e) = w.send(Message::Close(None)).await {
                        tracing::error!("Error sending close: {:?}", e);
                    }
                }
            })
            .abort_on_drop(false);

            loop {
                match r.next().await {
                    Some(Ok(Message::Text(msg))) => {
                        let new_settings = serde_json::from_str(msg.as_str()).unwrap();
                        let mut lock = state.write();

                        match &mut lock.data {
                            Some(value) => {
                                value.settings = new_settings;
                            }
                            None => {
                                lock.data = Some(SettingsData {
                                    settings: new_settings.clone(),
                                    primary_camera_buffer: new_settings.telescope_config.primary_camera.clone().unwrap_or_default(),
                                    mount_buffer: new_settings.telescope_config.mount.clone().unwrap_or_default(),
                                    focuser_buffer: new_settings.telescope_config.focuser.clone().unwrap_or_default(),
                                    filter_wheel_buffer: new_settings.telescope_config.filter_wheel.clone().unwrap_or_default(),
                                    flat_panel_buffer: new_settings.telescope_config.flat_panel.clone().unwrap_or_default(),
                                    entries: new_settings,
                                })
                            }
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("Got error from websocket: {:?}", e);
                    }
                    _ => {
                        break;
                    }
                }
            }
            close_task.abort();
        }
    }
}

pub struct SettingsData {
    settings: Settings,
    entries: Settings,
    // String buffers for text editing (empty string represents None)
    primary_camera_buffer: String,
    mount_buffer: String,
    focuser_buffer: String,
    filter_wheel_buffer: String,
    flat_panel_buffer: String,
}


fn some_string(s: &String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.clone())
    }
}

impl Widget for &mut SettingsWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        match &mut self.data {
            Some(settings_data) => {
                ui.vertical(|ui| {
                    egui::Grid::new("device")
                        .num_columns(3)
                        .striped(false)
                        .show(ui, |ui| {
                            ui.label("Indi Server");
                            ui.label(&settings_data.settings.indi_server_addr);
                            ui.text_edit_singleline(&mut settings_data.entries.indi_server_addr);
                            ui.end_row();
                            ui.separator();
                            ui.end_row();

                            ui.label("Primary Camera");
                            ui.label(settings_data.settings.telescope_config.primary_camera.as_deref().unwrap_or(""));
                            ui.text_edit_singleline(&mut settings_data.primary_camera_buffer);
                            ui.end_row();

                            ui.label("Mount");
                            ui.label(settings_data.settings.telescope_config.mount.as_deref().unwrap_or(""));
                            ui.text_edit_singleline(&mut settings_data.mount_buffer);
                            ui.end_row();

                            ui.label("Focuser");
                            ui.label(settings_data.settings.telescope_config.focuser.as_deref().unwrap_or(""));
                            ui.text_edit_singleline(&mut settings_data.focuser_buffer);
                            ui.end_row();

                            ui.label("Filter Wheel");
                            ui.label(settings_data.settings.telescope_config.filter_wheel.as_deref().unwrap_or(""));
                            ui.text_edit_singleline(&mut settings_data.filter_wheel_buffer);
                            ui.end_row();

                            ui.label("Flat Panel");
                            ui.label(settings_data.settings.telescope_config.flat_panel.as_deref().unwrap_or(""));
                            ui.text_edit_singleline(&mut settings_data.flat_panel_buffer);
                            ui.end_row();
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            let mut settings = settings_data.entries.clone();
                            settings.telescope_config.primary_camera = some_string(&settings_data.primary_camera_buffer);
                            settings.telescope_config.mount = some_string(&settings_data.mount_buffer);
                            settings.telescope_config.focuser = some_string(&settings_data.focuser_buffer);
                            settings.telescope_config.filter_wheel = some_string(&settings_data.filter_wheel_buffer);
                            settings.telescope_config.flat_panel = some_string(&settings_data.flat_panel_buffer);

                            spawn((), |_| {
                                async move {
                                    let client = reqwest::Client::new();

                                    let response =
                                        client.post(post_url()).json(&settings).send().await;

                                    match response {
                                        Ok(response) => {
                                            if !response.status().is_success() {
                                                tracing::error!(
                                                    "HTTP error: {}",
                                                    response.status()
                                                );
                                                return;
                                            }
                                        }
                                        Err(e) => tracing::error!("Error making count: {:?}", e),
                                    }
                                }
                            })
                            .abort_on_drop(false);
                        }
                    })
                })
                .response
            }
            None => ui.spinner(),
        }
    }
}
