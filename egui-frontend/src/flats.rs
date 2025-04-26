use crate::{get_websocket_base, Agent};
use egui::{ScrollArea, TextStyle, Window};
use futures::SinkExt;
use futures::{executor::block_on, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio_tungstenite_wasm::Message;
use twinkle_api::flats::{Config, FlatRun, MessageToServer};
use twinkle_client::{
    notify::Notify,
    task::{spawn, Abortable, Task},
};

type FlatState = Arc<Notify<FlatWidget>>;

impl crate::Widget for &mut Config {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::Grid::new("config")
            .num_columns(2)
            // .spacing([40.0, 40.0])
            .striped(false)
            .show(ui, |ui| {
                ui.label("Count");
                ui.add(egui::DragValue::new(&mut self.count).range(0u16..=u16::MAX));
                ui.end_row();

                ui.label("Filter");

                for (filter, selected) in self.filters.iter_mut() {
                    ui.toggle_value(selected, &filter.name);
                }
                ui.end_row();

                ui.label("Target ADU");
                ui.add(egui::DragValue::new(&mut self.adu_target).range(0u16..=u16::MAX));
                ui.end_row();

                ui.label("ADU Margin");
                ui.add(egui::DragValue::new(&mut self.adu_margin).range(0u16..=u16::MAX));
                ui.end_row();
                ui.label("Binning");

                for (bin, selected) in self.binnings.iter_mut() {
                    ui.toggle_value(selected, format!("bin{}", bin));
                }
                ui.end_row();

                ui.label("Gain");
                ui.add(egui::DragValue::new(&mut self.gain).range(0..=500));
                ui.end_row();

                ui.label("Offset");
                ui.add(egui::DragValue::new(&mut self.offset).range(0..=500));
                ui.end_row();
            })
            .response
    }
}

impl crate::Widget for &FlatRun {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(egui::widgets::ProgressBar::new(self.progress))
    }
}
pub struct FlatWidget {
    config: Config,
    status: twinkle_client::task::Status<FlatRun>,
    sender: tokio::sync::mpsc::Sender<MessageToServer>,
    messages: Vec<String>,
}

impl FlatWidget {
    fn new(sender: tokio::sync::mpsc::Sender<MessageToServer>) -> Self {
        FlatWidget {
            config: Config {
                count: 30,
                filters: vec![].into_iter().collect(),
                adu_target: u16::MAX / 2,
                adu_margin: u16::MAX,
                binnings: vec![(1, false), (2, true)].into_iter().collect(),
                gain: 120.,
                offset: 10.,
                exposure: Duration::from_secs(3),
            },
            status: twinkle_client::task::Status::Completed,
            sender,
            messages: Default::default(),
        }
    }
}

impl crate::Widget for &FlatState {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut state = block_on(self.write());
        ui.vertical(|ui| {
            state.config.ui(ui);
            ui.separator();
            if let twinkle_client::task::Status::Running(status) = &state.status {
                if ui.button("Stop").clicked() {
                    state.sender.try_send(MessageToServer::Stop).unwrap();
                }
                status.ui(ui);
            } else {
                if ui.button("Start").clicked() {
                    state
                        .sender
                        .try_send(MessageToServer::Start(state.config.clone()))
                        .unwrap();
                }
            }
            ui.separator();
            let text_style = TextStyle::Body;
            let row_height = ui.text_style_height(&text_style);
            let num_rows = state.messages.len();

            ScrollArea::vertical().auto_shrink(false).stick_to_bottom(true).show_rows(
                ui,
                row_height,
                num_rows,
                |ui, row_range| {
                    for row in &state.messages[row_range] {
                        ui.label(row);
                    }
                },
            );

        })
        .response
    }
}

pub struct FlatManager {
    flats: Option<Agent<FlatState>>,
}

fn get_websocket_url() -> String {
    format!("{}flats", get_websocket_base())
}

impl FlatManager {
    pub fn new() -> Self {
        FlatManager { flats: None }
    }

    pub fn windows(&mut self, ui: &mut egui::Ui) {
        if let Some(flats) = &self.flats {
            let mut open = true;
            Window::new("flats")
                .open(&mut open)
                .resizable(true)
                .scroll([false, false])
                .show(ui.ctx(), |ui| {
                    ui.add(flats);
                });
            if !open {
                flats.abort();
            }
        }
    }

    async fn task(
        state: FlatState,
        ctx: egui::Context,
        mut rs: tokio::sync::mpsc::Receiver<MessageToServer>,
    ) {
        let websocket = match tokio_tungstenite_wasm::connect(get_websocket_url()).await {
            Ok(websocket) => websocket,
            Err(e) => {
                tracing::error!("Failed to connect to {}: {:?}", get_websocket_url(), e);
                return;
            }
        };
        let (mut ws_write, mut ws_read) = websocket.split();
        let reader = async move {
            loop {
                match ws_read.next().await {
                    Some(Ok(Message::Text(msg))) => {
                        let msg: twinkle_api::flats::MessageToClient =
                            serde_json::from_str(msg.as_str()).unwrap();
                        match msg {
                            twinkle_api::flats::MessageToClient::Parameterization(
                                parameterization,
                            ) => {
                                let mut state = state.write().await;
                                state.config.filters = parameterization
                                    .filters
                                    .into_iter()
                                    .map(|filter| (filter, false))
                                    .collect();
                            }
                            twinkle_api::flats::MessageToClient::Status(status) => {
                                match status.into() {
                                    Ok(updated_status) => {
                                        state.write().await.status = updated_status
                                    }
                                    Err(e) => {
                                        tracing::error!("Error syncing state: {:?}", e);
                                    }
                                }
                            }
                            twinkle_api::flats::MessageToClient::Log(msg) => {
                                tracing::warn!(msg);
                                state.write().await.messages.push(msg);
                            }
                        }
                        ctx.request_repaint();
                    }
                    _ => {
                        break;
                    }
                }
            }
        };

        let writer = async move {
            while let Some(msg) = rs.recv().await {
                tracing::info!("Sending: {:?}", msg);
                if let Err(e) = ws_write
                    .send(Message::Text(serde_json::to_string(&msg).unwrap()))
                    .await
                {
                    tracing::error!("Unable tos end message to server: {:?}", e);
                    break;
                }
            }
        };

        tokio::select! {
            _ = writer => {}
            _ = reader => {}
        };
    }
}

impl egui::Widget for &mut FlatManager {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        block_on(async {
            if let Some(flats) = &self.flats {
                if !flats.status().read().await.running() {
                    self.flats = None;
                }
            }
        });
        ui.vertical(|ui| match &self.flats {
            Some(flats) => {
                if ui.selectable_label(true, "Flats").clicked() {
                    flats.abort();
                }
            }
            None => {
                if ui.selectable_label(false, "Flats").clicked() {
                    let (tx, rs) = tokio::sync::mpsc::channel(10);
                    self.flats = Some(
                        spawn(
                            Arc::new(Notify::new(FlatWidget::new(tx))),
                            |state: &FlatState| {
                                FlatManager::task(state.clone(), ui.ctx().clone(), rs)
                            },
                        )
                        .into(),
                    )
                }
            }
        })
        .response
    }
}
