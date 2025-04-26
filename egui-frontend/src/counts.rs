use std::{
    collections::{BTreeMap, BTreeSet},
    ops::DerefMut,
    sync::Arc,
    time::Duration,
};

use egui::Window;
use futures::{executor::block_on, StreamExt};
use tokio::sync::Mutex;
use tokio_tungstenite_wasm::Message;
use twinkle_api::Count;
use twinkle_client::{notify::Notify, task::Task};
use twinkle_client::{
    sleep,
    task::{spawn, Abortable},
};
use uuid::Uuid;

use crate::{get_http_base, get_websocket_base, Agent};

pub struct CountWidget {
    id: Uuid,
    count: Option<Count>,
}

impl CountWidget {
    fn new(id: Uuid) -> Self {
        CountWidget { id, count: None }
    }
}
impl crate::Widget for &Arc<Mutex<CountWidget>> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        block_on(async move {
            let mut binding = self.lock().await;
            let count_widget = binding.deref_mut();
            ui.vertical(|ui| match &count_widget.count {
                Some(count) => {
                    let count = count.count;
                    ui.label(format!("count: {}", count));
                    if ui.button("Done").clicked() {
                        let id = count_widget.id.clone();
                        spawn((), |_| async move {
                            destroy(id).await;
                        }).abort_on_drop(false);
                    }
                }
                None => {
                    ui.label("Connecting...");
                }
            })
            .response
        })
    }
}

pub struct CountIndex {
    ids: Arc<Mutex<BTreeSet<Uuid>>>,
    tasks: Arc<Notify<BTreeMap<Uuid, Agent<Arc<Mutex<CountWidget>>>>>>,
}

impl CountIndex {
    pub fn new(ctx: egui::Context) -> Self {
        let ids: Arc<Mutex<BTreeSet<Uuid>>> = Default::default();
        spawn(ids.clone(), |ids| {
            let ids = ids.clone();
            async move { Self::subscribe(ids, ctx).await }
        }).abort_on_drop(false);
        Self {
            ids,
            tasks: Default::default(),
        }
    }

    pub fn windows(&mut self, ui: &mut egui::Ui) {
        block_on(async { self.tasks.write().await }).retain(|id, agent| {
            if !block_on(async {
                let status = agent.status().read().await;
                status.running() || status.pending()
            }) {
                return false;
            }
            let mut open = true;
            Window::new(id.to_string())
                .open(&mut open)
                .resizable(true)
                .scroll([true, false])
                .show(ui.ctx(), |ui| {
                    ui.add(agent);
                });
            open
        });
    }

    #[tracing::instrument(skip_all)]
    async fn subscribe(tasks: Arc<Mutex<BTreeSet<Uuid>>>, ctx: egui::Context) {
        loop {
            let websocket = match tokio_tungstenite_wasm::connect(get_websocket_url()).await {
                Ok(websocket) => websocket,
                Err(e) => {
                    tracing::error!("Failed to connect to {}: {:?}", get_websocket_url(), e);
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            let (_ws_write, mut ws_read) = websocket.split();

            loop {
                match ws_read.next().await {
                    Some(Ok(Message::Text(msg))) => {
                        *tasks.lock().await = serde_json::from_str(msg.as_str()).unwrap();
                    }
                    _ => {
                        tasks.lock().await.clear();
                        break;
                    }
                }
                ctx.request_repaint();
            }
        }
    }
}

impl egui::Widget for &mut CountIndex {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| {
            block_on(async move {
                if ui.button("Count!").clicked() {
                    twinkle_client::task::spawn_with_value(
                        async move { crate::counts::create().await },
                    ).abort_on_drop(false);
                }
                if ui.button("Open all").clicked() {
                    for id in self.ids.lock().await.iter() {
                        if !self.tasks.read().await.contains_key(id) {
                            self.tasks
                                .write()
                                .await
                                .insert(id.clone(), new(id.clone(), ui.ctx().clone()));
                        }
                    }
                }
                if ui.button("Close all").clicked() {
                    self.tasks.write().await.clear();
                }
                for id in self.ids.lock().await.iter() {
                    let running = self.tasks.read().await.contains_key(id);
                    if ui.selectable_label(running, id.to_string()).clicked() {
                        if !running {
                            self.tasks
                                .write()
                                .await
                                .insert(id.clone(), new(id.clone(), ui.ctx().clone()));
                        } else {
                            self.tasks.write().await.remove(id);
                        }
                    }
                }
            })
        })
        .response
    }
}

fn get_websocket_url() -> String {
    format!("{}counts", get_websocket_base())
}

fn get_websocket_url_id(id: &Uuid) -> String {
    format!("{}counts/{}", get_websocket_base(), id)
}

fn get_create_url() -> String {
    format!("{}counts", get_http_base())
}
fn get_delete_url(id: &Uuid) -> String {
    format!("{}counts/{}", get_http_base(), id)
}

#[tracing::instrument(skip_all)]
pub async fn create() {
    let client = reqwest::Client::new();

    // Use a relative path - reqwest will use the current origin in a WASM context
    let response = client.post(get_create_url()).send().await;

    match response {
        Ok(response) => {
            if !response.status().is_success() {
                // You might want to handle this differently
                tracing::error!("HTTP error: {}", response.status());
                return;
            }
        }
        Err(e) => tracing::error!("Error making count: {:?}", e),
    }
}

#[tracing::instrument(skip_all)]
pub async fn destroy(id: Uuid) {
    let client = reqwest::Client::new();

    // Use a relative path - reqwest will use the current origin in a WASM context
    let response = client.delete(get_delete_url(&id)).send().await;

    match response {
        Ok(response) => {
            if !response.status().is_success() {
                // You might want to handle this differently
                tracing::error!("HTTP error: {}", response.status());
                return;
            }
        }
        Err(e) => tracing::error!("Error making count: {:?}", e),
    }
}

#[tracing::instrument(skip_all)]
pub fn new(id: Uuid, ctx: egui::Context) -> Agent<Arc<Mutex<CountWidget>>> {
    let state: Arc<Mutex<CountWidget>> = Arc::new(Mutex::new(CountWidget::new(id)));
    spawn(state, |state| {
        let state = state.clone();
        async move {
            let websocket = match tokio_tungstenite_wasm::connect(get_websocket_url_id(&id)).await {
                Ok(websocket) => websocket,
                Err(e) => {
                    tracing::error!(
                        "Failed to connect to {}: {:?}",
                        get_websocket_url_id(&id),
                        e
                    );
                    return;
                }
            };
            let (_ws_write, mut ws_read) = websocket.split();
            loop {
                match ws_read.next().await {
                    Some(Ok(Message::Text(msg))) => {
                        let count: Count = serde_json::from_str(msg.as_str()).unwrap();
                        {
                            let mut lock = state.lock().await;
                            lock.count.replace(count);
                        }
                        ctx.request_repaint();
                    }
                    _ => {
                        break;
                    }
                }
            }
        }
    })
    .into()
}
