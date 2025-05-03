use eframe::glow;
use egui::{ahash::HashMap, Context, ScrollArea, TextStyle};
use futures::executor::block_on;
use indi::{
    client::{AsyncClientConnection, Client, ClientTask},
    serialization::{EnableBlob, SetBlobVector},
};
use itertools::Itertools;
use tokio::sync::Mutex;
use twinkle_client::{
    task::{spawn, Abortable},
    OnDropFutureExt,
};
use uuid::Uuid;

use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    sync::Arc,
};
use strum::Display;
use tokio_stream::StreamExt;
use tracing::error;

use crate::Agent;
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
const DRIVERS: [&str; 13] = [
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
    id: Uuid,
    _client_task: ClientTask<()>,
    devices_tab_view: TabView,
    device_entries: HashMap<String, crate::indi::views::device::Device>,
    messages: VecDeque<String>,
    logs_tab_view: TabView,
    control: control::Control<{ DRIVERS.len() }>,
}
#[derive(Default)]
pub struct State {
    connection_status: ConnectionStatus,
}
fn get_websocket_url() -> String {
    format!("{}indi", get_websocket_base())
}

#[tracing::instrument(skip_all)]
async fn process_set_blob_vector(
    sbv: &mut SetBlobVector,
    state: Arc<Mutex<State>>,
    glow: Option<std::sync::Arc<glow::Context>>,
) {
    for blob in sbv.blobs.iter_mut() {
        let image_name = format!("{}.{}", sbv.name, blob.name);

        if blob.format == "download" {
            {
                let mut state = state.lock().await;
                if let ConnectionStatus::Connected(connection) = &mut state.connection_status {
                    if let Some(device) = connection.device_entries.get_mut(&sbv.device) {
                        device
                            .download_image(
                                image_name.clone(),
                                <std::option::Option<Arc<eframe::glow::Context>> as Clone>::clone(
                                    &glow,
                                )
                                .unwrap()
                                .deref(),
                                String::from_utf8_lossy(&blob.value.0).to_string(),
                            )
                            .await;
                    }
                }
            }
        }
    }
}

#[tracing::instrument(skip_all)]
async fn task(state: Arc<Mutex<State>>, ctx: Context, glow: Option<std::sync::Arc<glow::Context>>) {
    let url = get_websocket_url();

    {
        state.lock().await.connection_status = ConnectionStatus::Connecting;
    }

    let websocket = match tokio_tungstenite_wasm::connect(url).await {
        Ok(websocket) => websocket,
        Err(e) => {
            error!("Failed to connect to {}: {:?}", get_websocket_url(), e);
            return;
        }
    };
    let (w, r) = websocket.to_indi();
    let (def_blob_sender, mut def_blob_receiver) = tokio::sync::mpsc::unbounded_channel();

    let r = r.filter_map({
        let ctx = ctx.clone();
        let state = state.clone();
        move |x| {
            ctx.request_repaint();
            if let Ok(cmd) = &x {
                if let Some(message) = cmd.message() {
                    if message.len() > 0 {
                        block_on(async {
                            let mut state = state.lock().await;
                            if let ConnectionStatus::Connected(connection) =
                                &mut state.connection_status
                            {
                                connection.messages.push_back(message.clone());
                            }
                        });
                    }
                }
            }
            match x {
                Ok(indi::serialization::Command::DefBlobVector(dbv)) => {
                    let _ = def_blob_sender.send(dbv.clone());
                    Some(Ok(indi::serialization::Command::DefBlobVector(dbv)))
                }
                Ok(indi::serialization::Command::SetBlobVector(mut sbv)) => {
                    spawn((), {
                        let state = state.clone();
                        let glow = glow.clone();
                        |_| async move { process_set_blob_vector(&mut sbv, state, glow).await }
                    })
                    .abort_on_drop(false);
                    None
                }
                Ok(indi::serialization::Command::Message(msg)) => {
                    block_on(async {
                        let mut state = state.lock().await;
                        if let ConnectionStatus::Connected(connection) =
                            &mut state.connection_status
                        {
                            if let Some(message) = msg.message {
                                connection.messages.push_back(message);
                            }
                        }
                    });
                    None
                }
                Ok(cmd) => Some(Ok(cmd)),
                Err(e) => Some(Err(e)),
            }
        }
    });
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let client = Client::new(Some(tx.clone()));
    let connected = client.get_connected();
    let devices_sub = { client.get_devices().subscribe().await };
    let client_task = indi::client::start_with_streams(&client, rx, w, r, None, None);
    let update_device_list = {
        let state = state.clone();
        let mut sub = devices_sub;

        async move {
            loop {
                let devices = match sub.next().await {
                    Some(Ok(devices)) => devices,
                    _ => break,
                };
                let mut lock = state.lock().await;
                if let ConnectionStatus::Connected(connection) = &mut lock.connection_status {
                    for device_name in devices.keys() {
                        let device = client.device::<()>(device_name.as_str()).await;
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
                }
            }
        }
    };
    let enable_blobs = {
        async move {
            loop {
                let dbv = match def_blob_receiver.recv().await {
                    Some(msg) => msg,
                    None => return,
                };
                {
                    let _ = tx.send(indi::serialization::Command::EnableBlob(EnableBlob {
                        device: dbv.device.clone(),
                        name: Some(dbv.name.clone()),
                        enabled: indi::BlobEnable::Also,
                    }));
                }
            }
        }
    };

    let still_connected = async move {
        let mut stream = connected.subscribe().await;
        loop {
            let connected = match stream.next().await {
                Some(Ok(c)) => *c,
                _ => false,
            };
            if !connected {
                break;
            }
        }
    };
    {
        state.lock().await.connection_status = ConnectionStatus::Connected(Connection {
            id: Uuid::new_v4(),
            _client_task: client_task,
            devices_tab_view: Default::default(),
            device_entries: Default::default(),
            messages: Default::default(),
            logs_tab_view: Default::default(),
            control: control::Control {
                drivers: DRIVERS,
                current_driver: 0,
            },
        });
    }
    tokio::select! {
        _ = still_connected => { },
        _ = enable_blobs => {}
        _ = update_device_list => {}
    };
    ctx.request_repaint();
}

pub fn new(ctx: Context, glow: Option<std::sync::Arc<glow::Context>>) -> Agent<Arc<Mutex<State>>> {
    let state = Arc::new(Mutex::new(Default::default()));
    spawn(state, |state| {
        task(state.clone(), ctx, glow).on_drop({
            let state = state.clone();
            move || {
                let mut status = block_on(state.lock());
                status.connection_status = ConnectionStatus::Disconnected;
            }
        })
    })
    .into()
}

impl crate::Widget for &Arc<Mutex<State>> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut status = block_on(self.lock());
        let state = status.deref_mut();
        match &mut state.connection_status {
            ConnectionStatus::Disconnected => ui.label("Disconnected"),
            ConnectionStatus::Connecting => ui.spinner(),
            ConnectionStatus::Connected(connection) => {
                egui::TopBottomPanel::bottom(connection.id.to_string())
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
