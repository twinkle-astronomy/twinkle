use crate::agent::Agent;
use crate::agent::AgentLock;
use crate::agent::Widget;
use crate::get_websocket_base;
use egui::Window;
use egui::{ScrollArea, TextStyle};
use futures::SinkExt;
use futures::StreamExt;
use twinkle_client::task::Abortable;
use twinkle_client::task::IsRunning;

use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite_wasm::Message;
use twinkle_api::flats::MessageToServer;



#[derive(derive_more::Deref, derive_more::DerefMut, derive_more::AsRef, derive_more::AsMut, derive_more::From, Debug)]
pub struct Config(twinkle_api::flats::Config);


impl egui::Widget for &mut Config {
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


#[derive(derive_more::Deref, derive_more::DerefMut, derive_more::AsRef, derive_more::AsMut, derive_more::From, derive_more::Into, Debug)]
pub struct FlatRun(twinkle_api::flats::FlatRun);

impl egui::Widget for &FlatRun {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(egui::widgets::ProgressBar::new(self.progress))
    }
}
pub struct FlatWidget {
    config: Config,
    status: twinkle_client::task::Status<twinkle_api::flats::FlatRun>,
    sender: tokio::sync::mpsc::Sender<MessageToServer>,
    messages: Vec<String>,
}

impl FlatWidget {
    fn new(sender: tokio::sync::mpsc::Sender<MessageToServer>) -> Self {
        FlatWidget {
            config: twinkle_api::flats::Config {
                count: 30,
                filters: vec![].into_iter().collect(),
                adu_target: u16::MAX / 2,
                adu_margin: u16::MAX,
                binnings: vec![(1, false), (2, true)].into_iter().collect(),
                gain: 120.,
                offset: 10.,
                exposure: Duration::from_secs(3),
            }.into(),
            status: twinkle_client::task::Status::Completed,
            sender,
            messages: Default::default(),
        }
    }
}

impl Widget for &mut FlatWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| {
            ui.add(&mut self.config);
            ui.separator();
            if let twinkle_client::task::Status::Running(status) = &self.status {
                if ui.button("Stop").clicked() {
                    self.sender.try_send(MessageToServer::Stop).ok();
                }
                ui.add(&FlatRun(status.clone()));
            } else {
                if ui.button("Start").clicked() {
                    self.sender
                        .try_send(MessageToServer::Start(self.config.clone())).ok();
                }
            }
            ui.separator();
            let text_style = TextStyle::Body;
            let row_height = ui.text_style_height(&text_style);
            let num_rows = self.messages.len();

            ScrollArea::vertical()
                .auto_shrink(false)
                .stick_to_bottom(true)
                .show_rows(ui, row_height, num_rows, |ui, row_range| {
                    for row in &self.messages[row_range] {
                        ui.label(row);
                    }
                });
        })
        .response
    }
}

fn get_websocket_url() -> String {
    format!("{}flats", get_websocket_base())
}


async fn task(
    state: Arc<AgentLock<FlatWidget>>,
    mut rx: tokio::sync::mpsc::Receiver<MessageToServer>,
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
                    let msg: twinkle_api::flats::MessageToClient = serde_json::from_str(msg.as_str()).unwrap();

                    match msg {
                        twinkle_api::flats::MessageToClient::Parameterization(parameterization) => {
                            state.write().config.filters = parameterization
                                .filters
                                .into_iter()
                                .map(|filter| (filter, false))
                                .collect();
                        }
                        twinkle_api::flats::MessageToClient::Status(status) => {
                            match status.into() {
                                Ok(updated_status) => state.write().status = updated_status,
                                Err(e) => {
                                    tracing::error!("Error syncing state: {:?}", e);
                                }
                            }
                        }
                        twinkle_api::flats::MessageToClient::Log(msg) => {
                            state.write().messages.push(msg);
                        }
                    }                   
                }
                _ => {
                    break;
                }
            }
        }
    };

    let writer = async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = ws_write
                .send(Message::Text(serde_json::to_string(&msg).unwrap()))
                .await
            {
                tracing::error!("Unable to send message to server: {:?}", e);
                break;
            }
        }
    };

    tokio::select! {
        _ = writer => {}
        _ = reader => {}
    };
}


#[derive(Default)]
pub struct FlatManager {
    agent: Agent<FlatWidget>
}
impl FlatManager {
    pub fn windows(&mut self, ui: &mut egui::Ui) {
        if self.agent.running() {
            let mut open = true;
            Window::new("Flats")
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
impl egui::Widget for &mut FlatManager {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| match self.agent.running() {
            true => {
                if ui.selectable_label(true, "Flats").clicked() {
                    self.agent.abort();
                }
            }
            false => {
                if ui.selectable_label(false, "Flats").clicked() {
                    let (tx, rx) = tokio::sync::mpsc::channel(10);
                    self.agent.spawn(ui.ctx().clone(), FlatWidget::new(tx), |state| task(state, rx));
                }
            }
        })
        .response
    }
}
