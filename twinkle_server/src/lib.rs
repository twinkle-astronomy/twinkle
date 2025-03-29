use std::{collections::HashMap, sync::Arc};

use indi::IndiConnectionData;
use tokio::sync::RwLock;
use uuid::Uuid;
use twinkle_client::{notify::Notify, task::AsyncTask};

pub mod indi;
pub mod counts;

#[derive(Default, Clone)]
pub struct AppState {
    // Store device data by device name
    store: Arc<RwLock<StateData>>,
}

#[derive(Default)]
struct StateData {
    connections: HashMap<Uuid, Arc<RwLock<IndiConnectionData>>>,
    runs: HashMap<Uuid, AsyncTask<(), Arc<Notify<twinkle_api::Count>>>>,
}
