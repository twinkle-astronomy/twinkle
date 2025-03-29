use std::{ops::Deref, sync::Arc};

use futures::{executor::block_on, StreamExt};
use tokio::sync::Mutex;
use tokio_tungstenite_wasm::Message;
use twinkle_api::{Count, CreateCountResponse};
use twinkle_client::task::{spawn, Abortable};
use uuid::Uuid;

use crate::{get_websocket_base, Agent};

impl crate::Widget for &Arc<Mutex<Option<Count>>> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        block_on(async move {
            match self.lock().await.deref() {
                Some(count) => {
                    let count = count.count;
                    ui.label(format!("count: {}", count))
                },
                None => {
                    ui.label("Connecting...")
                },
            }
        })
    }
}

fn get_websocket_url(id: &Uuid) -> String {
    format!("{}counts/{}", get_websocket_base(), id)
}

fn get_create_url() -> String {
    format!("{}counts", get_websocket_base())
}

pub fn new() -> Agent<(), Arc<Mutex<Option<Count>>>> {
    let state = Arc::new(Mutex::new(None));
    spawn(state, |state| {
        let state = state.clone();
        async move {
            let client = reqwest::Client::new();

            // Use a relative path - reqwest will use the current origin in a WASM context
            let response = client.post(get_create_url()).send().await.unwrap();

            if !response.status().is_success() {
                // You might want to handle this differently
                tracing::error!("HTTP error: {}", response.status());
                return
            }

            let count: CreateCountResponse = response.json().await.unwrap();
            
            let websocket = match tokio_tungstenite_wasm::connect(get_websocket_url(&count.id)).await {
                Ok(websocket) => websocket,
                Err(e) => {
                    tracing::error!(
                        "Failed to connect to {}: {:?}",
                        get_websocket_url(&count.id),
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
                            lock.replace(count);
                        }
                    },
                    _ => break,
                }
            }
        }
    })
    .abort_on_drop(true)
    .into()
}
