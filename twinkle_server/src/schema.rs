// @generated automatically by Diesel CLI.

diesel::table! {
    use crate::sqlite_mapping::*;

    settings (rowid) {
        rowid -> Integer,
        indi_server_addr -> Text,
        telescope_config_id -> Integer,
    }
}

diesel::table! {
    use crate::sqlite_mapping::*;

    telescope_configs (rowid) {
        rowid -> Integer,
        mount -> Text,
        primary_camera -> Text,
        focuser -> Text,
        filter_wheel -> Text,
        flat_panel -> Text,
    }
}

diesel::joinable!(settings -> telescope_configs (telescope_config_id));

diesel::allow_tables_to_appear_in_same_query!(
    settings,
    telescope_configs,
);
