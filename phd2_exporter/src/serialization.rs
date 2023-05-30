use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Version {
    // #[serde(flatten)]
    // pub common: Common,
    #[serde(alias = "PHDVersion")]
    pub phd_version: String,
    #[serde(alias = "PHDSubver")]
    pub phd_subver: String,
    #[serde(alias = "OverlapSupport")]
    pub overlap_support: bool,
    #[serde(alias = "MsgVersion")]
    pub msg_version: u32,
}

#[derive(Deserialize, Debug)]
pub struct LockPositionSet {
    #[serde(alias = "X")]
    pub x: f64,
    #[serde(alias = "Y")]
    pub y: f64,
}

#[derive(Deserialize, Debug)]
pub struct Calibrating {
    #[serde(alias = "Mount")]
    pub mount: String,
    pub dir: String,
    pub dx: f64,
    pub dy: f64,
    pub pos: [f64; 2],
    pub step: f64,
    #[serde(alias = "State")]
    pub state: String,
}

#[derive(Deserialize, Debug)]
pub struct CalibrationComplete {
    #[serde(alias = "Mount")]
    pub mount: String,
}

#[derive(Deserialize, Debug)]
pub struct StarSelected {
    #[serde(alias = "X")]
    pub x: f64,
    #[serde(alias = "Y")]
    pub y: f64,
}

#[derive(Deserialize, Debug)]
pub struct StartGuiding {}

#[derive(Deserialize, Debug)]
pub struct Paused {}

#[derive(Deserialize, Debug)]
pub struct StartCalibration {
    #[serde(alias = "Mount")]
    pub mount: String,
}

#[derive(Deserialize, Debug)]
pub enum State {
    Stopped,
    Selected,
    Calibrating,
    Guiding,
    LostLock,
    Paused,
    Looping,
}
#[derive(Debug)]
pub struct InvalidState(String);

impl TryFrom<&str> for State {
    type Error = InvalidState;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Stoppped" => Ok(State::Stopped),
            "Selected" => Ok(State::Selected),
            "Calibrating" => Ok(State::Calibrating),
            "Guiding" => Ok(State::Guiding),
            "LostLock" => Ok(State::LostLock),
            "Paused" => Ok(State::Paused),
            "Looping" => Ok(State::Looping),
            other => Err(InvalidState(String::from(other))),
        }
    }
}
#[derive(Deserialize, Debug)]
pub struct AppState {
    #[serde(alias = "State")]
    pub state: State,
}

#[derive(Deserialize, Debug)]
pub struct CalibrationFailed {
    #[serde(alias = "Timestamp")]
    #[serde(alias = "Reason")]
    pub reason: String,
}

#[derive(Deserialize, Debug)]
pub struct CalibrationDataFlipped {
    #[serde(alias = "Mount")]
    pub mount: String,
}

#[derive(Deserialize, Debug)]
pub struct LockPositionShiftLimitReached {}

#[derive(Deserialize, Debug)]
pub struct LoopingExposures {
    #[serde(alias = "Frame")]
    pub frame: u32,
}

#[derive(Deserialize, Debug)]
pub struct LoopingExposuresStopped {}

#[derive(Deserialize, Debug)]
pub struct SettleBegin {}

#[derive(Deserialize, Debug)]
pub struct Settling {
    #[serde(alias = "Distance")]
    pub distance: f64,
    #[serde(alias = "Time")]
    pub time: f64,
    #[serde(alias = "SettleTime")]
    pub settle_time: f64,
    #[serde(alias = "StarLocked")]
    pub star_locked: bool,
}

#[derive(Deserialize, Debug)]
pub struct SettleDone {
    #[serde(alias = "Status")]
    pub status: u32,
    #[serde(alias = "Error")]
    pub error: String,
    #[serde(alias = "TotalFrames")]
    pub total_frames: u32,
    #[serde(alias = "DroppedFrames")]
    pub dropped_frames: u32,
}

#[derive(Deserialize, Debug)]
pub struct StarLost {
    #[serde(alias = "Frame")]
    pub frame: u32,
    #[serde(alias = "Time")]
    pub time: f64,
    #[serde(alias = "StarMass")]
    pub star_mass: f64,
    #[serde(alias = "SNR")]
    pub snr: f64,
    #[serde(alias = "AvgDist")]
    pub avg_dist: f64,
    #[serde(alias = "ErrorCode")]
    pub error_code: i32,
    #[serde(alias = "Status")]
    pub status: String,
}

#[derive(Deserialize, Debug)]
pub struct GuidingStopped {}

#[derive(Deserialize, Debug)]
pub struct Resumed {}

#[derive(Deserialize, Debug)]
pub enum NorthSouth {
    North,
    South,
}
#[derive(Deserialize, Debug)]
pub enum EastWest {
    East,
    West,
}

#[derive(Deserialize, Debug)]
pub struct GuideStep {
    #[serde(alias = "Frame")]
    pub frame: u32,
    #[serde(alias = "Time")]
    pub time: f64,
    #[serde(alias = "Mount")]
    pub mount: String,
    pub dx: f64,
    pub dy: f64,
    #[serde(alias = "RADistanceRaw")]
    pub ra_distance_raw: f64,
    #[serde(alias = "DECDistanceRaw")]
    pub de_distance_raw: f64,
    #[serde(alias = "RADistanceGuide")]
    pub ra_distance_guide: f64,
    #[serde(alias = "DECDistanceGuide")]
    pub de_distance_guide: f64,
    #[serde(alias = "RADuration")]
    pub ra_duration: Option<f64>,
    #[serde(alias = "RADirection")]
    pub ra_direction: Option<EastWest>,
    #[serde(alias = "DECDuration")]
    pub dec_duration: Option<f64>,
    #[serde(alias = "DECDirection")]
    pub dec_direction: Option<NorthSouth>,
    #[serde(alias = "StarMass")]
    pub star_mass: f64,
    #[serde(alias = "SNR")]
    pub snr: f64,
    #[serde(alias = "HFD")]
    pub hfd: f64,
    #[serde(alias = "AvgDist")]
    pub avg_dist: f64,
    #[serde(alias = "RALimited")]
    pub ra_limited: Option<bool>,
    #[serde(alias = "DecLimited")]
    pub dec_limited: Option<f64>,
    #[serde(alias = "ErrorCode")]
    pub error_code: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct GuidingDithered {
    pub dx: f64,
    pub dy: f64,
}

#[derive(Deserialize, Debug)]
pub struct LockPositionLost {}

#[derive(Deserialize, Debug)]
pub struct Alert {
    #[serde(alias = "Msg")]
    pub msg: String,
    #[serde(alias = "Type")]
    pub msg_type: String,
}

#[derive(Deserialize, Debug)]
pub struct GuideParamChange {
    #[serde(alias = "Name")]
    pub name: String,
    #[serde(alias = "Value")]
    pub value: serde_json::Value,
}

#[derive(Deserialize, Debug)]
pub struct ConfigurationChange {}

#[derive(Deserialize, Debug)]
#[serde(tag = "Event")]
pub enum Event {
    Version(Version),
    LockPositionSet(LockPositionSet),
    Calibrating(Calibrating),
    CalibrationComplete(CalibrationComplete),
    StarSelected(StarSelected),
    StartGuiding(StartGuiding),
    Paused(Paused),
    StartCalibration(StartCalibration),
    AppState(AppState),
    CalibrationFailed(CalibrationFailed),
    CalibrationDataFlipped(CalibrationDataFlipped),
    LockPositionShiftLimitReached(LockPositionShiftLimitReached),
    LoopingExposures(LoopingExposures),
    LoopingExposuresStopped(LoopingExposuresStopped),
    SettleBegin(SettleBegin),
    Settling(Settling),
    SettleDone(SettleDone),
    StarLost(StarLost),
    GuidingStopped(GuidingStopped),
    Resumed(Resumed),
    GuideStep(GuideStep),
    GuidingDithered(GuidingDithered),
    LockPositionLost(LockPositionLost),
    Alert(Alert),
    GuideParamChange(GuideParamChange),
    ConfigurationChange(ConfigurationChange),
}

#[derive(Deserialize, Debug)]
pub struct ServerEvent {
    #[serde(alias = "Timestamp")]
    pub timestamp: f64,
    #[serde(alias = "Host")]
    pub host: String,
    #[serde(alias = "Inst")]
    pub inst: u32,

    #[serde(flatten)]
    pub event: Event,
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,

    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ServerMessage {
    ServerEvent(ServerEvent),
    JsonRpcResponse(JsonRpcResponse),
}

#[derive(Serialize, Debug)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,

    pub params: serde_json::Value,
}

#[derive(Debug)]
pub struct DurationSeconds(Duration);

impl From<Duration> for DurationSeconds {
    fn from(value: Duration) -> Self {
        DurationSeconds(value)
    }
}
impl Serialize for DurationSeconds {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_f64(self.0.as_secs_f64())
    }
}
