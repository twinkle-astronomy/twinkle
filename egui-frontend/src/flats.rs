use crate::sync_task::{SyncAble, SyncTask};
use crate::get_websocket_base;
use egui::{ScrollArea, TextStyle, Widget};
use futures::SinkExt;
use futures::StreamExt;

use std::time::Duration;
use tokio_tungstenite_wasm::Message;
use twinkle_api::flats::{MessageToClient, MessageToServer};



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
    // sender: tokio::sync::mpsc::Sender<MessageToServer>,
    messages: Vec<String>,
}

impl Default for FlatWidget {
    fn default() -> Self {
        tracing::info!("new FlatWidget");
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
            messages: Default::default(),
        }
    }
}

impl SyncAble for FlatWidget {
    type MessageFromTask = twinkle_api::flats::MessageToClient;
    type MessageToTask = twinkle_api::flats::MessageToServer;

    fn update(&mut self, msg: Self::MessageFromTask) {
        match msg {
            twinkle_api::flats::MessageToClient::Parameterization(parameterization) => {
                self.config.filters = parameterization
                    .filters
                    .into_iter()
                    .map(|filter| (filter, false))
                    .collect();
            }
            twinkle_api::flats::MessageToClient::Status(status) => {
                match status.into() {
                    Ok(updated_status) => self.status = updated_status,
                    Err(e) => {
                        tracing::error!("Error syncing state: {:?}", e);
                    }
                }
            }
            twinkle_api::flats::MessageToClient::Log(msg) => {
                // tracing::warn!(msg);
                self.messages.push(msg);
            }
        }
    }


    fn ui(&mut self, ui: &mut egui::Ui, tx: tokio::sync::mpsc::UnboundedSender<MessageToServer>) -> egui::Response {
        ui.vertical(|ui| {
            self.config.ui(ui);
            ui.separator();
            if let twinkle_client::task::Status::Running(status) = &self.status {
                if ui.button("Stop").clicked() {
                    tx.send(MessageToServer::Stop).unwrap();
                }
                ui.add(&FlatRun(status.clone()));
            } else {
                if ui.button("Start").clicked() {
                    tx
                        .send(MessageToServer::Start(self.config.clone()))
                        .unwrap();
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

    fn window_name(&self) -> impl Into<egui::WidgetText> {
        "Flats"
    }
}

fn get_websocket_url() -> String {
    format!("{}flats", get_websocket_base())
}


async fn task(
    tx: crate::sync_task::Sender<MessageToClient>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<MessageToServer>,
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

                    if let Err(e) = tx.send(msg) {
                        tracing::error!("Unable to send message to client: {:?}", e);
                        break;
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
            tracing::info!("Sending: {:?}", msg);
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

impl egui::Widget for &mut SyncTask<FlatWidget> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| match self.running() {
            true => {
                if ui.selectable_label(true, "Flats").clicked() {
                    self.abort();
                }
            }
            false => {
                if ui.selectable_label(false, "Flats").clicked() {
                    self.spawn(|tx, rx| task(tx, rx));
                }
            }
        })
        .response
    }
}
