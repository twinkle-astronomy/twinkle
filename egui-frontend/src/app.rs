use egui::{Context, ScrollArea};
use futures::executor::block_on;
use itertools::Itertools;
use log::info;
use parking_lot::Mutex;
use std::sync::Arc;
use strum::Display;
use tokio_stream::StreamExt;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    selected_device: String,

    #[serde(skip)]
    connection: Arc<Mutex<ClientConnection>>,
}

#[derive(Default)]
struct ClientConnection {
    status: ConnectionStatus,
}

struct ClientState {
    client: indi::client::Client,
}

#[derive(Display)]
enum ConnectionStatus {
    Disconnecting,
    Disconnected,
    Connecting,
    Connected(ClientState),
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        ConnectionStatus::Disconnected
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            connection: Default::default(),
            selected_device: "".to_string(),
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let this: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        info!("Spawning - Connecting");

        wasm_bindgen_futures::spawn_local(Self::connect(
            this.connection.clone(),
            cc.egui_ctx.clone(),
        ));
        this
    }

    async fn connect(connection: Arc<Mutex<ClientConnection>>, ctx: Context) {
        info!("Connecting");
        {
            connection.lock().status = ConnectionStatus::Connecting
        };
        let websocket = tokio_tungstenite_wasm::connect("ws://localhost:4000/indi")
            .await
            .unwrap();
        info!("Got connection");
        let client = indi::client::new(websocket, None, None).unwrap();
        let mut sub = client.get_devices().subscribe().await;
        {
            let mut lock = connection.lock();
            lock.status = ConnectionStatus::Connected(ClientState { client });
        };

        loop {
            ctx.request_repaint();
            match sub.next().await {
                Some(Ok(_)) => {}
                Some(Err(_)) => break,
                None => break,
            };
        }
        {
            let mut lock = connection.lock();
            lock.status = ConnectionStatus::Disconnected;
        };
    }

    async fn disconnect(connection: Arc<Mutex<ClientConnection>>, ctx: Context) {
        info!("Disconnecting");

        {
            connection.lock().status = ConnectionStatus::Disconnecting
        };

        {
            let mut lock = connection.lock();
            if let ConnectionStatus::Connected(state) = &mut lock.status {
                state.client.shutdown();
            }
            lock.status = ConnectionStatus::Disconnected;
        };
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            {
                let mut connection = self.connection.lock();
                ui.label(connection.status.to_string());

                match &mut connection.status {
                    ConnectionStatus::Disconnected => {
                        if ui.button("Connect").clicked() {
                            wasm_bindgen_futures::spawn_local(Self::connect(
                                self.connection.clone(),
                                ctx.clone(),
                            ));
                        }
                    }
                    ConnectionStatus::Connecting => {
                        ui.label("Connecting... ");
                        ui.spinner();
                    }
                    ConnectionStatus::Connected(state) => {
                        if ui.button("Disconnect").clicked() {
                            wasm_bindgen_futures::spawn_local(Self::disconnect(
                                self.connection.clone(),
                                ctx.clone(),
                            ))
                        }
                        ui.horizontal(|ui| {
                            for device in
                                block_on(state.client.get_devices().lock()).keys().sorted()
                            {
                                ui.selectable_value(
                                    &mut self.selected_device,
                                    device.clone(),
                                    device,
                                );
                            }
                        });
                        if let Some(device) = block_on(async {
                            state
                                .client
                                .device::<()>(self.selected_device.as_str())
                                .await
                        }) {
                            ui.separator();
                            ScrollArea::vertical()
                                .max_height(ui.available_height())
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    ui.add(crate::indi::widgets::Device::new(&device));
                                });
                        }
                    }
                    ConnectionStatus::Disconnecting => {
                        ui.label("Disconnecting... ");
                        ui.spinner();
                    }
                }
            }
            ui.separator();

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
