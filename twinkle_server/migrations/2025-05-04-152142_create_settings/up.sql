-- Your SQL goes here
CREATE TABLE telescope_configs (
    mount TEXT NOT NULL,
    primary_camera TEXT NOT NULL,
    focuser TEXT NOT NULL,
    filter_wheel TEXT NOT NULL,
    flat_panel TEXT NOT NULL
);

-- Create settings table with foreign key to telescope_configs
CREATE TABLE settings (
    indi_server_addr TEXT NOT NULL,
    telescope_config_id INTEGER NOT NULL,
    FOREIGN KEY (telescope_config_id) REFERENCES telescope_configs(rowid)
);
