use eframe::glow;
use egui::{ahash::HashMap, Context, ScrollArea};
use futures::executor::block_on;
use indi::{
    client::AsyncClientConnection,
    serialization::{EnableBlob, SetBlobVector},
};
use itertools::Itertools;
use tokio::sync::Mutex;
use twinkle_client::OnDropFutureExt;

use std::{ops::{Deref, DerefMut}, sync::Arc};
use strum::Display;
use tokio_stream::StreamExt;
use tracing::{debug, error};
use url::form_urlencoded;

use crate::task::{spawn, AsyncTask};

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

struct Connection {
    client: indi::client::Client,
    devices_tab_view: TabView,
    device_entries: HashMap<String, crate::indi::views::device::Device>,
}

#[derive(Default)]
pub struct State {
    connection_status: ConnectionStatus,
}

impl Drop for State {
    fn drop(&mut self) {
        debug!("Dropping indi agent state");
    }
}

#[cfg(debug_assertions)]
fn get_websocket_url(encoded_value: &str) -> String {
    format!("ws://localhost:4000/indi?server_addr={}", encoded_value)
}

#[cfg(not(debug_assertions))]
fn get_websocket_url(encoded_value: &str) -> String {
    // format!("/indi?server_addr={}", encoded_value)
    format!("ws://localhost:4000/indi?server_addr={}", encoded_value)
}

#[tracing::instrument(skip_all)]
async fn process_set_blob_vector(
    mut sbv: SetBlobVector,
    state: Arc<Mutex<State>>,
    glow: Option<std::sync::Arc<glow::Context>>,
) {
    debug!("enter process_set_blob_vector");
    for blob in sbv.blobs.iter_mut() {
        let image_name = format!("{}.{}", sbv.name, blob.name);

        if blob.format == "download" {
            {
                let mut state = state.lock().await;
                if let ConnectionStatus::Connected(connection) = &mut state.connection_status {
                    if let Some(device) = connection.device_entries.get_mut(&sbv.device) {
                        device.download_image(
                            image_name.clone(),
                            <std::option::Option<Arc<eframe::glow::Context>> as Clone>::clone(
                                &glow,
                            )
                            .unwrap()
                            .deref(),
                            String::from_utf8_lossy(&blob.value.0).to_string(),
                        ).await;
                    }
                }
            }
        }
    }

    debug!("exit process_set_blob_vector");
}

#[tracing::instrument(skip_all)]
async fn task(
    state: Arc<Mutex<State>>,
    server_addr: String,
    ctx: Context,
    glow: Option<std::sync::Arc<glow::Context>>,
) {
    let encoded_value = form_urlencoded::byte_serialize(server_addr.as_bytes()).collect::<String>();
    let url = get_websocket_url(&encoded_value);
    debug!("Connecting to {}", url);

    {
        state.lock().await.connection_status = ConnectionStatus::Connecting;
    }

    let websocket = match tokio_tungstenite_wasm::connect(url).await {
        Ok(websocket) => websocket,
        Err(e) => {
            error!(
                "Failed to connect to {}: {:?}",
                get_websocket_url(&encoded_value),
                e
            );
            return;
        }
    };
    let (w, r) = websocket.to_indi();
    let (def_blob_sender, mut def_blob_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (blob_send, mut blob_recv) = tokio::sync::mpsc::channel(1);

    let blob_receiver =  {
        let glow = glow.clone();
        let state = state.clone();
        async move {
            debug!("start blob_receiver");
            loop {
                let sbv: SetBlobVector = match blob_recv.recv().await {
                    Some(m) => m,
                    None => break,
                };
                process_set_blob_vector(sbv, state.clone(), glow.clone()).await;
            }
            debug!("end blob_receiver");
        }
        .on_drop(|| debug!("drop blob_receiver"))
    };

    let r = r.filter({
        let ctx = ctx.clone();
        move |x| {
            ctx.request_repaint();
            match x {
                Ok(indi::serialization::Command::DefBlobVector(dbv)) => {
                    let _ = def_blob_sender.send(dbv.device.clone());
                    true
                }
                Ok(indi::serialization::Command::SetBlobVector(sbv)) => {
                    let _ = blob_send.try_send(sbv.clone());
                    false
                }
                _ => true,
            }
        }
    });
    let client = match indi::client::new_with_streams(w, r, None, None) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to build indi client: {:?}", e);
            state.lock().await.connection_status = ConnectionStatus::Disconnected;
            return;
        }
    };
    let command_sender = client.command_sender();
    let join = client.join();
    {
        state.lock().await.connection_status = ConnectionStatus::Connected(Connection {
            client,
            devices_tab_view: Default::default(),
            device_entries: Default::default(),
        });
    }
    let enable_blobs = {
        async move {
            loop {
                let msg = match def_blob_receiver.recv().await {
                    Some(msg) => msg,
                    None => return,
                };
                {
                    if let Some(ref cs) = command_sender {
                        let _ = cs.send(indi::serialization::Command::EnableBlob(EnableBlob {
                            device: msg,
                            name: None,
                            enabled: indi::BlobEnable::Also,
                        }));
                    }
                }
            }
        }
    };
    tokio::select! {
        _ = join => {debug!("join finished")},
        _ = enable_blobs => {debug!("enable_blobs finished")}
        _ = blob_receiver => {debug!("blob_receiver finished")}
    };
    ctx.request_repaint();
}

pub fn new(
    server_addr: String,
    ctx: Context,
    glow: Option<std::sync::Arc<glow::Context>>,
) -> AsyncTask<(), Arc<Mutex<State>>> {
    let state = Arc::new(Mutex::new(Default::default()));
     spawn(state, |state| {
            task(state.clone(), server_addr, ctx, glow)
        }).abort_on_drop(true)
    
}


impl crate::Widget for &Arc<Mutex<State>> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut status = block_on(self.lock());
        let state = status.deref_mut();
        match &mut state.connection_status {
            ConnectionStatus::Disconnected => {
                ui.label("Disconnected")
            }
            ConnectionStatus::Connecting => {
                ui.spinner()
            }
            ConnectionStatus::Connected(connection) => {
                let selected = connection.devices_tab_view.show(
                    ui,
                    block_on(connection.client.get_devices().lock())
                        .keys()
                        .sorted(),
                );
                if let Some(selected) = selected {
                    if let Some(device) =
                        block_on(async { connection.client.device::<()>(selected.as_str()).await })
                    {
                        ui.vertical(|ui| {
                            let device_view = connection
                            .device_entries
                            .entry(selected.clone())
                            .or_insert_with(|| Device::new(device.clone()));
                        ui.separator();
                        ScrollArea::vertical()
                            .max_height(ui.available_height())
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                ui.add(device_view);
                            });
                        }).response
                    } else {
                        ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
                    }
                } else {
                    ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
                }
            }
        }   
    }
}

