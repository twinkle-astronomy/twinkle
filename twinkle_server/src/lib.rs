use std::{collections::HashMap, sync::Arc};

use indi::IndiConnectionData;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod indi;

#[derive(Debug, Default, Clone)]
pub struct AppState {
    // Store device data by device name
    store: Arc<RwLock<StateData>>,
}

#[derive(Debug, Default)]
struct StateData {
    connections: HashMap<Uuid, Arc<RwLock<IndiConnectionData>>>,
}
