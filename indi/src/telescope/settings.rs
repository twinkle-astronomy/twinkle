use serde::{Deserialize, Serialize};

/// Type used to name the indi devices that coorespond to
/// various parts of the telescope.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TelescopeConfig {
    pub mount: Option<String>,
    pub primary_camera: Option<String>,
    pub focuser: Option<String>,
    pub filter_wheel: Option<String>,
    pub flat_panel: Option<String>,
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
                mount: Some(String::from("Telescope Simulator")),
                primary_camera: Some(String::from("CCD Simulator")),
                focuser: Some(String::from("Focuser Simulator")),
                filter_wheel: Some(String::from("Filter Simulator")),
                flat_panel: Some(String::from("Light Panel Simulator")),
            },
        }
    }
}
