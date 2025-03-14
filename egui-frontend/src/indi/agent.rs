use eframe::glow;
use egui::{ahash::HashMap, mutex::Mutex, Context, ScrollArea};
use futures::executor::block_on;
use indi::{
    client::AsyncClientConnection,
    serialization::EnableBlob,
};
use itertools::Itertools;
use log::{error, info};
use std::{ops::Deref, sync::Arc};
use strum::Display;
use tokio_stream::StreamExt;
use url::form_urlencoded;

use crate::{
    app::Agent,
    task::{spawn, AsyncTask, Task},
};

use super::views::tab::TabView;

pub struct IndiAgent {
    task: AsyncTask<()>,
    state: Arc<Mutex<State>>,
}

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
struct State {
    connection_status: ConnectionStatus,
}

#[cfg(debug_assertions)]
fn get_websocket_url(encoded_value: &str) -> String {
    format!("ws://localhost:4000/indi?server_addr={}", encoded_value)
}

#[cfg(not(debug_assertions))]
fn get_websocket_url(encoded_value: &str) -> String {
    format!("/indi?server_addr={}", encoded_value)
}

async fn reader(
    state: Arc<Mutex<State>>,
    server_addr: String,
    ctx: Context,
    glow: Option<std::sync::Arc<glow::Context>>,
) {
    let state = state.clone();
    let encoded_value = form_urlencoded::byte_serialize(server_addr.as_bytes()).collect::<String>();
    let url = get_websocket_url(&encoded_value);
    info!("Connecting to {}", url);

    {
        state.lock().connection_status = ConnectionStatus::Connecting;
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
    let (snd, mut recv) = tokio::sync::mpsc::unbounded_channel();
    let r = r.filter({
        let ctx = ctx.clone();
        let state = state.clone();
        move |x| {
            ctx.request_repaint();
            match x {
                Ok(indi::serialization::Command::DefBlobVector(dbv)) => {
                    let _ = snd.send(dbv.device.clone());
                    true
                }
                Ok(indi::serialization::Command::SetBlobVector(sbv)) => {
                    info!("GOT BLOB!");
                    let mut state = state.lock();
                    if let ConnectionStatus::Connected(connection) = &mut state.connection_status {
                        info!("getting device entry: {}", &sbv.device);
                        if let Some(device) = connection.device_entries.get_mut(&sbv.device) {
                            info!("got entry");

                            device.set_blob(
                                sbv,
                                <std::option::Option<Arc<eframe::glow::Context>> as Clone>::clone(
                                    &glow,
                                )
                                .unwrap()
                                .deref(),
                            );
                        }
                    }
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
            state.lock().connection_status = ConnectionStatus::Disconnected;
            return;
        }
    };
    let command_sender = client.command_sender();
    let join = client.join();
    {
        state.lock().connection_status = ConnectionStatus::Connected(Connection {
            client,
            devices_tab_view: Default::default(),
            device_entries: Default::default(),
        });
    }
    let enable_blobs = {
        async move {
            loop {
                let msg = match recv.recv().await {
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
        _ = join => {info!("join finished")},
        _ = enable_blobs => {info!("enable_blobs finished")}
    };
    ctx.request_repaint();
}

pub fn new(
    server_addr: String,
    ctx: Context,
    glow: Option<std::sync::Arc<glow::Context>>,
) -> IndiAgent {
    let state: Arc<Mutex<State>> = Default::default();
    IndiAgent {
        task: spawn(reader(state.clone(), server_addr, ctx, glow)),
        state,
    }
}

impl Agent for IndiAgent {
    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        let mut state = self.state.lock();
        if let Some(gl) = gl {
            if let ConnectionStatus::Connected(connection) = &mut state.connection_status {
                for device in connection.device_entries.values_mut() {
                    device.on_exit(gl);
                }
                connection.device_entries.clear();
            }    
        }
    }
    fn show(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut state = self.state.lock();
        match &mut state.connection_status {
            ConnectionStatus::Disconnected => {
                ui.label("Disconnected");
            }
            ConnectionStatus::Connecting => {
                ui.spinner();
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
                        let device_view = connection
                            .device_entries
                            .entry(selected.clone())
                            .or_insert_with(Default::default);
                        ui.separator();
                        ScrollArea::vertical()
                            .max_height(ui.available_height())
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                device_view.show(ui, &device);
                            });
                    }
                }
            }
        }
    }
}

impl Task for IndiAgent {
    fn abort(&self) {
        self.task.abort()
    }

    fn status(&self) -> crate::task::Status {
        self.task.status()
    }
}
