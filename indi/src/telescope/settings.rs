use serde::{Deserialize, Serialize};

/// Type used to name the indi devices that coorespond to 
/// various parts of the telescope.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TelescopeConfig {
    pub mount: String,
    pub primary_camera: String,
    pub focuser: String,
    pub filter_wheel: String,
    pub flat_panel: String,
}

/// Type used to describe the configuration of a telescope
/// including the indi address to connect too, and device names
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Settings {
    pub indi_server_addr: String,
    pub telescope_config: TelescopeConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            indi_server_addr: "indi:7624".to_string(),
            telescope_config: TelescopeConfig {
                mount: String::from("Telescope Simulator"),
                primary_camera: String::from("CCD Simulator"),
                focuser: String::from("Focuser Simulator"),
                filter_wheel: String::from("Filter Simulator"),
                flat_panel: String::from("Light Panel Simulator"),
            },
        }
    }
}
