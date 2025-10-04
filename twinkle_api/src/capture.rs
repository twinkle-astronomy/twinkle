use std::time::Duration;

use serde::{Deserialize, Serialize};
use twinkle_client::{
    notify::NotifyArc,
    task::{Status, TaskStatusError},
};

#[derive(Debug, Serialize, Deserialize)]
pub enum CaptureRequest {
    Start(CaptureConfig),
    Stop,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub exposure: Duration,
}

pub struct Image {
    pub url: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CaptureProgress {
    pub progress: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExposureParameterization {
    pub min: Duration,
    pub max: Duration,
    pub step: Duration,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum MessageToClient {
    ExposureParameterization(ExposureParameterization),
    Progress(Status<Result<CaptureProgress, TaskStatusError>>),
}

impl From<Status<Result<NotifyArc<CaptureProgress>, TaskStatusError>>> for MessageToClient {
    fn from(value: Status<Result<NotifyArc<CaptureProgress>, TaskStatusError>>) -> Self {
        MessageToClient::Progress(value.map(|x| x.map(|x| x.into_inner())))
    }
}
