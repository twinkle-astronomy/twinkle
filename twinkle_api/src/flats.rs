use std::time::Duration;

use serde::{Deserialize, Serialize};
use twinkle_client::task::TaskStatusError;

use crate::Filter;


#[derive(Serialize, Deserialize, Debug)]
pub struct ConnectionParams {
    pub server_addr: String,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub enum MessageToClient {
    Parameterization(Parameterization),
    Status(twinkle_client::task::Status<Result<crate::flats::FlatRun, TaskStatusError>>),
    Log(String),
}

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub enum MessageToServer {
    Start(Config),
    Stop,
}


#[derive(Serialize, Deserialize, Default)]
#[derive(Debug, Clone)]
pub struct Parameterization {
    pub filters: Vec<Filter>,
    pub binnings: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub struct Config {
    pub count: usize,
    pub filters: Vec<(Filter, bool)>,
    pub adu_target: u16,
    pub adu_margin: u16,
    pub binnings: Vec<(u8, bool)>,
    pub gain: f64,
    pub offset: f64,
    pub exposure: Duration,
}

impl Config {
    pub fn total_images(&self) -> usize {
        self.count * self.filters.iter().filter(|(_, enabled)| *enabled ).count() * self.binnings.iter().filter(|(_, enabled)| *enabled ).count()
    }
}

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, Default)]
pub struct FlatRun {
    pub progress: f32,
}