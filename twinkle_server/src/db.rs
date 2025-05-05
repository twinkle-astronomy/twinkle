use twinkle_api::{Settings, TelescopeConfig};

use diesel_async::RunQueryDsl;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use diesel::{
    prelude::AsChangeset, sqlite::SqliteConnection, Connection, ConnectionError, ExpressionMethods, Insertable, QueryDsl, Queryable, Selectable, SelectableHelper
};
use diesel_async::{sync_connection_wrapper::SyncConnectionWrapper, AsyncConnection};

use crate::{schema::*, StateData};

// Establish a connection
pub async fn establish_connection(
    database_url: &str,
) -> Result<SyncConnectionWrapper<SqliteConnection>, ConnectionError> {
    SyncConnectionWrapper::<SqliteConnection>::establish(&database_url).await
}

#[derive(Debug)]
pub enum MigrationError {
    ConnectionError(ConnectionError),
    RunError(Box<(dyn std::error::Error + std::marker::Send + Sync + 'static)>),
}

impl From<ConnectionError> for MigrationError {
    fn from(value: ConnectionError) -> Self {
        MigrationError::ConnectionError(value)
    }
}

impl From<Box<(dyn std::error::Error + std::marker::Send + Sync + 'static)>> for MigrationError {
    fn from(value: Box<(dyn std::error::Error + std::marker::Send + Sync + 'static)>) -> Self {
        MigrationError::RunError(value)
    }
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");
pub fn run_migrations(database_url: &str) -> Result<(), MigrationError> {

    let mut conn = SqliteConnection::establish(database_url)?;
    conn.run_pending_migrations(MIGRATIONS)?;
    Ok(())
}

#[derive(Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = telescope_configs)]
#[diesel(primary_key(rowid))]
struct TelescopeConfigModel {
    pub rowid: i64,
    pub mount: String,
    pub primary_camera: String,
    pub focuser: String,
    pub filter_wheel: String,
    pub flat_panel: String,
}

#[derive(Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = settings)]
struct SettingsModel {
    pub rowid: i64,
    pub indi_server_addr: String,
    pub telescope_config_id: i64,
}

impl StateData {
    // Save or update settings
    pub async fn save_settings(
        &mut self,
        
        settings: &Settings,
    ) -> Result<(), diesel::result::Error> {
        AsyncConnection::transaction(&mut self.db, |conn| {
            Box::pin(async move {
                // Check if we already have settings
                let existing_settings = settings::table
                    .select(SettingsModel::as_select())
                    .first::<SettingsModel>(conn)
                    .await;

                let telescope_config_model = TelescopeConfigModel {
                    rowid: 0,
                    mount: settings.telescope_config.mount.clone(),
                    primary_camera: settings.telescope_config.primary_camera.clone(),
                    focuser: settings.telescope_config.focuser.clone(),
                    filter_wheel: settings.telescope_config.filter_wheel.clone(),
                    flat_panel: settings.telescope_config.flat_panel.clone(),
                };

                if let Ok(mut existing_settings) = existing_settings {
                    existing_settings.indi_server_addr = settings.indi_server_addr.clone();

                    // Update existing telescope config
                    diesel::update(telescope_configs::table)
                        .filter(telescope_configs::rowid.eq(existing_settings.telescope_config_id))
                        .set(telescope_config_model)
                        .execute(conn)
                        .await?;

                    diesel::update(settings::table)
                        .set(existing_settings)
                        .execute(conn)
                        .await?;
                } else {
                    // Insert new telescope config
                    let telescope_config_id = diesel::insert_into(telescope_configs::table)
                        .values(&telescope_config_model)
                        .returning(telescope_configs::rowid)
                        .get_result::<i64>(conn)
                        .await?;

                    let settings_model = SettingsModel {
                        rowid: 0,
                        indi_server_addr: settings.indi_server_addr.clone(),
                        telescope_config_id,
                    };

                    diesel::insert_into(settings::table)
                        .values(settings_model)
                        .execute(conn)
                        .await?;
                };

                Ok(())
            })
        })
        .await
    }


    // Load settings
    pub async fn load_settings(
        conn: &mut SyncConnectionWrapper<SqliteConnection>,
    ) -> Result<Settings, diesel::result::Error> {
        // Try to load settings
        let settings_model = settings::table.first::<SettingsModel>(conn).await?;

        // Load related telescope config
        let telescope_config_model = telescope_configs::table
            .filter(telescope_configs::rowid.eq(settings_model.telescope_config_id))
            .first::<TelescopeConfigModel>(conn)
            .await?;

        // Convert to your types
        Ok(Settings {
            indi_server_addr: settings_model.indi_server_addr,
            telescope_config: TelescopeConfig {
                mount: telescope_config_model.mount,
                primary_camera: telescope_config_model.primary_camera,
                focuser: telescope_config_model.focuser,
                filter_wheel: telescope_config_model.filter_wheel,
                flat_panel: telescope_config_model.flat_panel,
            },
        })
    }
}


#[cfg(test)]
mod test {
    use diesel::dsl::count;
    use diesel::result::Error;

    use super::*;

    #[tokio::test]
    async fn test_simple() {
        let filename = "/tmp/test_simple.sqlite";
        tokio::fs::remove_file(filename).await.ok();
        run_migrations(&filename).unwrap();

        let mut state = StateData::new(format!("sqlite://{}", filename).as_str()).await.unwrap();

        // Empty db
        let settings = StateData::load_settings(&mut state.db).await;
        assert!(matches!(settings, Err(Error::NotFound)));

        // Save first version
        let mut new_settings = Settings::default();
        state.save_settings(&new_settings).await.unwrap();

        let settings = StateData::load_settings(&mut state.db).await;
        assert_eq!(settings, Ok(new_settings.clone()));

        // Save update
        new_settings.indi_server_addr = "somethingelse".to_string();
        state.save_settings(&new_settings).await.unwrap();

        let settings = StateData::load_settings(&mut state.db).await;
        assert_eq!(settings, Ok(new_settings.clone()));

        // We didn't create another record
        assert_eq!(
            1,
            settings::table
                .select(count(settings::rowid))
                .first::<i64>(&mut state.db)
                .await
                .unwrap()
        );
        assert_eq!(
            1,
            telescope_configs::table
                .select(count(telescope_configs::rowid))
                .first::<i64>(&mut state.db)
                .await
                .unwrap()
        );

    }

}
