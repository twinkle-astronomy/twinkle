use bytes::BytesMut;
use eframe::glow;
use egui::{ahash::HashMap, ScrollArea, TextStyle};
use futures::executor::block_on;
use indi::{
    client::{AsyncClientConnection, AsyncReadConnection, AsyncWriteConnection, Client},
    serialization::{device::DeviceUpdate, Command, EnableBlob, GetProperties, SetBlobVector},
};
use itertools::Itertools;
use ndarray::ArrayD;
use tokio_stream::StreamExt;
use twinkle_api::{analysis::Statistics, indi::api::ImageResponse};
use uuid::Uuid;

use std::{collections::VecDeque, sync::Arc};
use strum::Display;
use tracing::{error, Instrument};

use crate::{
    agent::{Agent, AgentLock},
    fits::image_view::ImageView,
    get_websocket_base,
    indi::{control, views::image_device::ImageDevice},
};

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

pub trait DeviceEntry {
    fn show(&mut self, ui: &mut egui::Ui) -> egui::Response;
    fn get_or_create_render(&mut self, _name: String, _gl: &glow::Context) -> &mut ImageView {
        unimplemented!()
    }
}

struct Connection {
    client: Client,
    devices_tab_view: TabView,
    device_entries: HashMap<String, Box<dyn DeviceEntry + Sync + Send>>,
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
    connection_status: ConnectionStatus,
    images: bool,
}

impl State {
    fn new(images: bool) -> Self {
        State {
            id: Uuid::new_v4(),
            connection_status: Default::default(),
            images,
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

fn get_websocket_url() -> String {
    format!("{}indi", get_websocket_base())
}

impl egui::Widget for &mut State {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        match &mut self.connection_status {
            ConnectionStatus::Disconnected => ui.label("Disconnected"),
            ConnectionStatus::Connecting => ui.spinner(),
            ConnectionStatus::Connected(connection) => {
                if self.images {
                    let selected = connection
                        .devices_tab_view
                        .show(ui, connection.device_entries.keys().sorted());
                    if let Some(selected) = selected {
                        if let Some(device_view) = connection.device_entries.get_mut(selected) {
                            ScrollArea::both()
                                .max_height(ui.available_height())
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        device_view.show(ui);
                                    })
                                })
                                .inner
                                .response
                        } else {
                            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
                        }
                    } else {
                        ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
                    }
                } else {
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
                                                ui.vertical(|ui| {
                                                    device_view.show(ui);
                                                })
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
}

#[tracing::instrument(skip_all)]
async fn process_set_blob_vector(
    state: Arc<AgentLock<State>>,
    mut sbv: SetBlobVector,
    glow: Option<std::sync::Arc<glow::Context>>,
) {
    for blob in sbv.blobs.iter_mut() {
        let device_name = sbv.device.clone();
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

                            let mut lock = state.write();
                            block_on({
                                let device_name = device_name.clone();
                                let image_name = image_name.clone();
                                let glow = glow.clone().unwrap();
                                async move {
                                    if let ConnectionStatus::Connected(connection) =
                                        &mut lock.connection_status
                                    {
                                        let device = connection
                                            .device_entries
                                            .get_mut(&device_name)
                                            .unwrap();
                                        let renderer =
                                            device.get_or_create_render(image_name, &glow);
                                        renderer.set_progress(progress);
                                    }
                                }
                            });
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
                let mut lock = state.write();
                block_on({
                    let device_name = device_name.clone();
                    let image_name = image_name.clone();
                    let glow = glow.clone().unwrap();
                    async move {
                        if let ConnectionStatus::Connected(connection) = &mut lock.connection_status
                        {
                            let device = connection.device_entries.get_mut(&device_name).unwrap();
                            let renderer = device.get_or_create_render(image_name, &glow);
                            renderer.set_image(data, resp.stats);
                        }
                    }
                });
            }
        }
    }
}

#[tracing::instrument(skip_all)]
async fn task(state: Arc<AgentLock<State>>) {
    state.write().connection_status = ConnectionStatus::Connecting;

    let websocket = match tokio_tungstenite_wasm::connect(get_websocket_url()).await {
        Ok(websocket) => websocket,
        Err(e) => {
            error!("Failed to connect to {}: {:?}", get_websocket_url(), e);
            return;
        }
    };

    let (mut w, mut r) = websocket.to_indi();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    state.write().connection_status = ConnectionStatus::Connected(Connection::new(tx));

    let reader_future = {
        async move {
            while let Some(cmd) = r.read().await {
                match cmd {
                    Ok(cmd) => match cmd {
                        cmd => {
                            let mut lock = state.write();
                            block_on(async move {
                                if let ConnectionStatus::Connected(connection) =
                                    &mut lock.connection_status
                                {
                                    let cmd = match cmd {
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

                                    let device_name = cmd.device_name().cloned();
                                    if let Some(device_name) = device_name {
                                        match indi::client::DeviceStore::update(
                                            connection.client.get_devices(),
                                            cmd,
                                        )
                                        .await
                                        {
                                            Ok(Some(DeviceUpdate::AddParameter(_))) => {
                                                let device = connection
                                                    .client
                                                    .device::<()>(device_name.as_str())
                                                    .await;
                                                if let Some(device) = device {
                                                    connection
                                                    .device_entries
                                                    .entry(device_name.clone())
                                                    .or_insert_with(
                                                        || -> Box<dyn DeviceEntry + Sync + Send> {
                                                            Box::new(Device::new(device.clone()))
                                                        },
                                                    );
                                                }
                                            }
                                            Ok(Some(DeviceUpdate::DeleteParameter(_))) => {
                                                if !connection
                                                    .client
                                                    .get_devices()
                                                    .read()
                                                    .await
                                                    .contains_key(&device_name)
                                                {
                                                    connection.device_entries.remove(&device_name);
                                                }
                                            }
                                            Ok(Some(DeviceUpdate::UpdateParameter(_))) => {}
                                            Ok(None) => {}
                                            Err(e) => {
                                                tracing::error!("Error updating devices: {:?}", e)
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    },
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

#[tracing::instrument(skip_all)]
async fn images_task(state: Arc<AgentLock<State>>, glow: Option<std::sync::Arc<glow::Context>>) {
    state.write().connection_status = ConnectionStatus::Connecting;

    let websocket = match tokio_tungstenite_wasm::connect(get_websocket_url()).await {
        Ok(websocket) => websocket,
        Err(e) => {
            error!("Failed to connect to {}: {:?}", get_websocket_url(), e);
            return;
        }
    };
    let (mut w, mut r) = websocket.to_indi();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    state.write().connection_status = ConnectionStatus::Connected(Connection::new(tx));

    let (sbv_tx, mut sbv_rx) = tokio::sync::mpsc::channel(1);

    let sbv_future = {
        let state = state.clone();
        let glow = glow.clone();
        async move {
            while let Some(sbv) = sbv_rx.recv().await {
                process_set_blob_vector(state.clone(), sbv, glow.clone()).await;
            }
        }
    };

    let reader_future = {
        async move {
            while let Some(cmd) = r.read().await {
                match cmd {
                    Ok(cmd) => match cmd {
                        indi::serialization::Command::SetBlobVector(sbv) => {
                            sbv_tx.try_send(sbv).ok();
                        }
                        cmd => {
                            let mut lock = state.write();
                            block_on(async move {
                                if let ConnectionStatus::Connected(connection) =
                                    &mut lock.connection_status
                                {
                                    let cmd = match cmd {
                                        indi::serialization::Command::DefBlobVector(dbv) => {
                                            if let Err(e) = connection.client.send(
                                                indi::serialization::Command::EnableBlob(
                                                    EnableBlob {
                                                        device: dbv.device.clone(),
                                                        name: Some(dbv.name.clone()),
                                                        enabled: indi::BlobEnable::Also,
                                                    },
                                                ),
                                            ) {
                                                tracing::error!("Error enabling blob: {:?}", e);
                                            }

                                            indi::serialization::Command::DefBlobVector(dbv)
                                        }
                                        cmd => cmd,
                                    };

                                    let device_name = cmd.device_name().cloned();
                                    if let Some(device_name) = device_name {
                                        match indi::client::DeviceStore::update(
                                            connection.client.get_devices(),
                                            cmd,
                                        )
                                        .await
                                        {
                                            Ok(Some(DeviceUpdate::AddParameter(_))) => {
                                                connection
                                                    .device_entries
                                                    .entry(device_name.clone())
                                                    .or_insert_with(
                                                        || Box::new(ImageDevice::new()),
                                                    );
                                            }
                                            Ok(Some(DeviceUpdate::DeleteParameter(_))) => {
                                                if !connection
                                                    .client
                                                    .get_devices()
                                                    .read()
                                                    .await
                                                    .contains_key(&device_name)
                                                {
                                                    connection.device_entries.remove(&device_name);
                                                }
                                            }
                                            Ok(Some(DeviceUpdate::UpdateParameter(_))) => {}
                                            Ok(None) => {}
                                            Err(e) => {
                                                tracing::error!("Error updating devices: {:?}", e)
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    },
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
        _ = sbv_future => {},
    }
}

pub fn new(
    images: bool,
    ctx: egui::Context,
    glow: Option<std::sync::Arc<glow::Context>>,
) -> Agent<State> {
    let state = State::new(images);
    let mut agent: Agent<State> = Default::default();
    agent.spawn(ctx, state, |state| {
        let glow = glow.clone();
        async move {
            if images {
                images_task(state, glow).await
            } else {
                task(state).await
            }
        }
    });
    agent
}
