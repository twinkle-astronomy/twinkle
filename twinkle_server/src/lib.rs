use std::{collections::HashMap, sync::Arc};

use db::{run_migrations, MigrationError};
use diesel::SqliteConnection;
use diesel_async::sync_connection_wrapper::SyncConnectionWrapper;
use indi::IndiConnectionData;
use tokio::sync::RwLock;
use twinkle_client::{agent::Agent, notify::Notify};
use uuid::Uuid;

use crate::telescope::Telescope;

pub mod db;
mod schema;
pub mod sqlite_mapping;

pub mod telescope;

pub mod flats;
pub mod indi;
pub mod settings;
pub mod tracing_broadcast;
pub mod capture;

pub mod websocket_handler;

#[derive(Clone)]
pub struct AppState {
    store: Arc<RwLock<StateData>>,
}

impl AppState {
    pub async fn new() -> Result<Self, MigrationError> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:///storage/db.sqlite".to_string());
        Ok(AppState {
            store: Arc::new(RwLock::new(StateData::new(database_url.as_str()).await?)),
        })
    }
}

struct StateData {
    connections: HashMap<Uuid, Arc<RwLock<IndiConnectionData>>>,
    flats: Arc<Notify<Agent<twinkle_api::flats::FlatRun>>>,
    settings: Arc<Notify<twinkle_api::settings::Settings>>,
    capture: Agent<twinkle_api::capture::CaptureProgress>,
    db: SyncConnectionWrapper<SqliteConnection>,
    telescope: Arc<RwLock<Telescope>>,
}

impl StateData {
    async fn new(filename: &str) -> Result<Self, MigrationError> {
        tokio::task::spawn_blocking({
            let filename = filename.to_string();
            move || run_migrations(filename.as_str())
        })
        .await
        .unwrap()?;

        let mut db = db::establish_connection(filename).await?;

        let settings = StateData::load_settings(&mut db).await.ok().unwrap_or_default();
        let telescope_config = settings.telescope_config.clone();
        Ok(StateData {
            connections: Default::default(),
            capture: Default::default(),
            flats: Default::default(),
            settings: Arc::new(Notify::new(settings)),
            db,
            telescope: Arc::new(RwLock::new(Telescope::new(telescope_config)))
        })
    }
}
