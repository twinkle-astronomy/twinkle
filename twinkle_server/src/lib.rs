use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures::{SinkExt, Stream, StreamExt};
use indi::IndiConnectionData;
use tokio::sync::RwLock;
use uuid::Uuid;
use twinkle_client::{agent::Agent, notify::Notify, task::AsyncTask};

pub mod telescope;

pub mod indi;
pub mod counts;
pub mod flats;
pub mod tracing_broadcast;
pub mod settings;

pub mod websocket_handler;

#[derive(Default, Clone)]
pub struct AppState {
    // Store device data by device name
    store: Arc<RwLock<StateData>>,
}

#[derive(Default)]
struct StateData {
    connections: HashMap<Uuid, Arc<RwLock<IndiConnectionData>>>,
    runs: Arc<Notify<HashMap<Uuid, AsyncTask<(), Arc<Notify<twinkle_api::Count>>>>>>,
    flats: Arc<Notify<Agent<twinkle_api::flats::FlatRun>>>,
    settings: Arc<Notify<twinkle_api::Settings>>,
}
