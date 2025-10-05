use std::time::Duration;

use serde::{Deserialize, Serialize};
use twinkle_client::task::TaskStatusError;

#[derive(Serialize, Deserialize, Debug)]
pub struct ConnectionParams {
    pub server_addr: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageToClient {
    Parameterization(Parameterization),
    Status(twinkle_client::task::Status<Result<crate::flats::FlatRun, TaskStatusError>>),
    Log(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageToServer {
    Start(Config),
    Stop,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Parameterization {
    pub filters: Vec<String>,
    pub binnings: Vec<u8>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum LightSource {
    FlatPanel(Duration),
    Sky {
        min_exposure: Duration,
        max_exposure: Duration,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub count: usize,
    pub filters: Vec<(String, bool)>,
    pub adu_target: u16,
    pub adu_margin: u16,
    pub binnings: Vec<(u8, bool)>,
    pub gain: f64,
    pub offset: f64,
    pub light_source: LightSource,
}

impl Config {
    pub fn total_images(&self) -> usize {
        self.count
            * self.filters.iter().filter(|(_, enabled)| *enabled).count()
            * self.binnings.iter().filter(|(_, enabled)| *enabled).count()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FlatRun {
    pub progress: f32,
}
