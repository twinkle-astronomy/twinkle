use bytes::BytesMut;
use eframe::glow;
use egui::{ahash::HashMap, Context, ScrollArea, TextStyle};
use futures::executor::block_on;
use indi::{
    client::{AsyncClientConnection, AsyncReadConnection, AsyncWriteConnection, Client},
    serialization::{Command, EnableBlob, GetProperties, SetBlobVector},
};
use itertools::Itertools;
use ndarray::ArrayD;
use tokio_stream::StreamExt;
use twinkle_api::{analysis::Statistics, indi::api::ImageResponse};
use twinkle_client::task::{spawn, Abortable};
use uuid::Uuid;

use std::collections::VecDeque;
use strum::Display;
use tracing::{error, Instrument};

use crate::sync_task::{Sender, SyncAble, SyncTask};
use crate::{get_websocket_base, indi::control};

use super::views::{device::Device, tab::TabView};

#[derive(Display)]
enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected(Connection),
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        ConnectionStatus::Disconnected
    }
}

#[cfg(not(debug_assertions))]
const DRIVERS: [&str; 6] = [
    "---",
    "indi_deepskydad_fp",
    "indi_asi_ccd",
    "indi_asi_focuser",
    "indi_asi_wheel",
    "indi_eqmod_telescope",
];

#[cfg(debug_assertions)]
const DRIVERS: [&str; 13] = [
    "---",
    "indi_simulator_ccd",
    "indi_simulator_dome",
    "indi_simulator_focus",
    "indi_simulator_gps",
    "indi_simulator_guide",
    "indi_simulator_lightpanel",
    "indi_simulator_receiver",
    "indi_simulator_rotator",
    "indi_simulator_sqm",
    "indi_simulator_telescope",
    "indi_simulator_weather",
    "indi_simulator_wheel",
];

struct Connection {
    client: Client,
    devices_tab_view: TabView,
    device_entries: HashMap<String, crate::indi::views::device::Device>,
    messages: VecDeque<String>,
    logs_tab_view: TabView,
    control: control::Control<{ DRIVERS.len() }>,
}

impl Connection {
    fn new(sender: tokio::sync::mpsc::UnboundedSender<Command>) -> Self {
        Connection {
            client: Client::new(Some(sender)),
            devices_tab_view: Default::default(),
            device_entries: Default::default(),
            messages: Default::default(),
            logs_tab_view: Default::default(),
            control: control::Control {
                drivers: DRIVERS,
                current_driver: 0,
            },
        }
    }
}

pub struct State {
    id: Uuid,
    glow: Option<std::sync::Arc<glow::Context>>,
    connection_status: ConnectionStatus,
    from_task_tx: Option<Sender<MessageFromTask>>,
    to_task_tx: Option<tokio::sync::mpsc::UnboundedSender<Command>>,
}

impl State {
    fn new(glow: Option<std::sync::Arc<glow::Context>>) -> Self {
        State {
            id: Uuid::new_v4(),
            glow,
            connection_status: Default::default(),
            from_task_tx: None,
            to_task_tx: None,
        }
    }
}

impl State {
    fn update_for_command(&mut self, cmd: Command) {
        if let ConnectionStatus::Connected(connection) = &mut self.connection_status {
            let cmd = match cmd {
                indi::serialization::Command::DefBlobVector(dbv) => {
                    if let Err(e) =
                        connection
                            .client
                            .send(indi::serialization::Command::EnableBlob(EnableBlob {
                                device: dbv.device.clone(),
                                name: Some(dbv.name.clone()),
                                enabled: indi::BlobEnable::Also,
                            }))
                    {
                        tracing::error!("Error enabling blob: {:?}", e);
                    }

                    indi::serialization::Command::DefBlobVector(dbv)
                }
                indi::serialization::Command::SetBlobVector(sbv) => {
                    spawn((), {
                        let tx = self.from_task_tx.clone().unwrap();
                        |_| async move { process_set_blob_vector(sbv, tx).await }
                    })
                    .abort_on_drop(false);
                    return;
                }
                indi::serialization::Command::Message(msg) => {
                    if let Some(message) = &msg.message {
                        connection.messages.push_back(message.clone());
                    }

                    indi::serialization::Command::Message(msg)
                }
                cmd => cmd,
            };

            if let Some(message) = cmd.message() {
                if message.len() > 0 {
                    connection.messages.push_back(message.clone());
                }
            }

            block_on(async move {
                if let Err(e) =
                    indi::client::DeviceStore::update(connection.client.get_devices(), cmd).await
                {
                    tracing::error!("Error updating devices: {:?}", e);
                }
                let devices = connection.client.get_devices().read().await;
                for device_name in devices.keys() {
                    let device = connection.client.device::<()>(device_name.as_str()).await;
                    if let Some(device) = device {
                        connection
                            .device_entries
                            .entry(device_name.clone())
                            .or_insert_with(|| Device::new(device.clone()));
                    }
                    connection
                        .device_entries
                        .retain(|k, _| devices.keys().any(|d| d == k));
                }
            });
        }
    }
}
pub enum MessageFromTask {
    Command(Command),
    Connected,
    DownloadProgress {
        device: String,
        name: String,
        progress: f32,
    },
    Image {
        device: String,
        name: String,
        data: ArrayD<u16>,
        stats: Statistics,
    },
}

impl SyncAble for State {
    type MessageFromTask = MessageFromTask;

    type MessageToTask = Command;

    fn reset(
        &mut self,
        to_task_tx: tokio::sync::mpsc::UnboundedSender<Self::MessageToTask>,
        from_task_tx: Sender<Self::MessageFromTask>,
    ) {
        self.to_task_tx = Some(to_task_tx);
        self.from_task_tx = Some(from_task_tx);
        self.connection_status = ConnectionStatus::Connecting;
    }

    fn update(&mut self, cmd: Self::MessageFromTask) {
        let tx = self.to_task_tx.as_ref().unwrap().clone();
        match cmd {
            MessageFromTask::Command(command) => self.update_for_command(command),
            MessageFromTask::Connected => {
                self.connection_status = ConnectionStatus::Connected(Connection::new(tx))
            }
            MessageFromTask::DownloadProgress {
                device,
                name,
                progress,
            } => {
                if let ConnectionStatus::Connected(connection) = &mut self.connection_status {
                    let device = connection.device_entries.get_mut(&device).unwrap();
                    let renderer = device.get_or_create_render(name, self.glow.as_ref().unwrap());
                    renderer.set_progress(progress);
                }
            }
            MessageFromTask::Image {
                device,
                name,
                data,
                stats,
            } => {
                if let ConnectionStatus::Connected(connection) = &mut self.connection_status {
                    let device = connection.device_entries.get_mut(&device).unwrap();
                    let renderer = device.get_or_create_render(name, self.glow.as_ref().unwrap());
                    renderer.set_image(data, stats);
                }
            }
        }
    }

    fn window_name(&self) -> impl Into<egui::WidgetText> {
        "Indi"
    }

    fn window_id(&self) -> egui::Id {
        self.id.to_string().into()
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _tx: tokio::sync::mpsc::UnboundedSender<Self::MessageToTask>,
    ) -> egui::Response {
        match &mut self.connection_status {
            ConnectionStatus::Disconnected => ui.label("Disconnected"),
            ConnectionStatus::Connecting => ui.spinner(),
            ConnectionStatus::Connected(connection) => {
                egui::TopBottomPanel::bottom(format!("bottom:{}", &self.id))
                    .resizable(false)
                    .min_height(0.0)
                    .show_inside(ui, |ui| {
                        let selected = connection
                            .logs_tab_view
                            .show(ui, ["Logs".to_string(), "Drivers".to_string()].iter());

                        if let Some(selected) = selected {
                            if selected == "Logs" {
                                let text_style = TextStyle::Body;
                                let row_height = ui.text_style_height(&text_style);
                                ScrollArea::vertical().auto_shrink(false).show_rows(
                                    ui,
                                    row_height,
                                    connection.messages.len(),
                                    |ui, row_range| {
                                        for row in connection.messages.range(row_range) {
                                            ui.label(format!("{:?}", row));
                                        }
                                    },
                                );
                            } else if selected == "Drivers" {
                                ui.add(&mut connection.control);
                            }
                        }
                    });

                egui::CentralPanel::default()
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let selected = connection
                                .devices_tab_view
                                .show(ui, connection.device_entries.keys().sorted());
                            if let Some(selected) = selected {
                                if let Some(device_view) =
                                    connection.device_entries.get_mut(selected)
                                {
                                    ui.separator();
                                    ScrollArea::both()
                                        .max_height(ui.available_height())
                                        .auto_shrink([false; 2])
                                        .show(ui, |ui| {
                                            ui.add(device_view);
                                        });
                                }
                            }
                        })
                    })
                    .response
            }
        }
    }
}
fn get_websocket_url() -> String {
    format!("{}indi", get_websocket_base())
}

#[tracing::instrument(skip_all)]
async fn process_set_blob_vector(mut sbv: SetBlobVector, tx: Sender<MessageFromTask>) {
    for blob in sbv.blobs.iter_mut() {
        let image_name = format!("{}.{}", sbv.name, blob.name);

        if blob.format == "download" {
            {
                let bytes = async {
                    let client = reqwest::Client::new();

                    // Use a relative path - reqwest will use the current origin in a WASM context
                    let response = client
                        .get(String::from_utf8_lossy(&blob.value.0).to_string())
                        .send()
                        .await
                        .unwrap();

                    if !response.status().is_success() {
                        // You might want to handle this differently
                        tracing::error!("HTTP error: {}", response.status());
                    }
                    let total_size = response.content_length().unwrap_or(0);

                    // Prepare a buffer for the data
                    let mut buffer = BytesMut::new();
                    let mut downloaded = 0;

                    // Get the response as a byte stream
                    let mut stream = response.bytes_stream();

                    // Process the stream chunk by chunk
                    while let Some(chunk) = stream.next().await {
                        let chunk = chunk.unwrap(); // Handle this error appropriately in production
                        downloaded += chunk.len() as u64;
                        buffer.extend_from_slice(&chunk);

                        // Calculate and report progress
                        if total_size > 0 {
                            let progress = (downloaded as f32) / (total_size as f32);
                            tx.send(MessageFromTask::DownloadProgress {
                                device: sbv.device.clone(),
                                name: image_name.clone(),
                                progress,
                            })
                            .unwrap();
                        }
                    }

                    buffer.to_vec()
                }
                .instrument(tracing::info_span!("download_indi_image"))
                .await;

                let resp: ImageResponse<'_> = {
                    let _span = tracing::info_span!("rmp_serde::from_slice").entered();
                    twinkle_api::indi::api::ImageResponse::from_bytes(bytes.as_ref()).unwrap()
                };

                let data = {
                    let _span = tracing::info_span!("read_fits").entered();
                    resp.image.read_image().unwrap()
                };
                tx.send(MessageFromTask::Image {
                    device: sbv.device.clone(),
                    name: image_name.clone(),
                    data,
                    stats: resp.stats,
                })
                .unwrap();
            }
        }
    }
}

#[tracing::instrument(skip_all)]
async fn task(
    tx: Sender<<State as SyncAble>::MessageFromTask>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<<State as SyncAble>::MessageToTask>,
) {
    let websocket = match tokio_tungstenite_wasm::connect(get_websocket_url()).await {
        Ok(websocket) => websocket,
        Err(e) => {
            error!("Failed to connect to {}: {:?}", get_websocket_url(), e);
            return;
        }
    };
    tx.send(MessageFromTask::Connected).unwrap();

    let (mut w, mut r) = websocket.to_indi();

    let reader_future = {
        async move {
            while let Some(cmd) = r.read().await {
                match cmd {
                    Ok(cmd) => {
                        tx.send(MessageFromTask::Command(cmd)).unwrap();
                    }
                    Err(e) => {
                        tracing::error!("Got error from indi server: {:?}", e);
                    }
                }
            }
        }
    };

    let writer_future = {
        async move {
            w.write(indi::serialization::Command::GetProperties(GetProperties {
                version: indi::INDI_PROTOCOL_VERSION.to_string(),
                device: None,
                name: None,
            }))
            .await
            .unwrap();

            while let Some(cmd) = rx.recv().await {
                w.write(cmd).await.unwrap();
            }
            if let Err(e) = w.shutdown().await {
                tracing::error!("Unable to shutdwon connection: {:?}", e);
            }
        }
    };

    tokio::select! {
        _ = reader_future => {},
        _ = writer_future => {},
    }
}

pub fn new(ctx: Context, glow: Option<std::sync::Arc<glow::Context>>) -> SyncTask<State> {
    // let state = Default::default();
    let mut sync_task: SyncTask<State> = SyncTask::new(State::new(glow), ctx);

    sync_task.spawn(|tx, rx| async move { task(tx, rx).await });
    sync_task
}
