use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod analysis;
pub mod capture;
pub mod fits;
pub mod indi;
pub mod flats;
pub mod settings;

#[derive(Serialize, Deserialize, Default)]
pub struct Count {
    pub  count: usize,
}
#[derive(Serialize, Deserialize)]
pub struct StreamCountRequestParams {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub struct CreateCountResponse {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, Eq)]
pub struct Filter {
    pub name: String,
    pub position: usize,
}
impl PartialEq for Filter {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.position == other.position
    }
}
impl PartialOrd for Filter {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.position.partial_cmp(&other.position)
    }
}

impl Ord for Filter {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.position.cmp(&other.position)
    }
}

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(f)
    }
}

impl From<&Filter> for f64 {
    fn from(value: &Filter) -> Self {
        value.position as f64
    }
}


#[cfg(target_arch = "wasm32")]
pub use tokio_tungstenite_wasm::Message;

#[cfg(not(target_arch = "wasm32"))]
pub use axum::extract::ws::Message;


pub trait ToWebsocketMessage {
    fn to_message(self) -> Message;
}

impl<T: Serialize> ToWebsocketMessage for T {
    fn to_message(self) -> Message {
        Message::Text(serde_json::to_string(&self).unwrap())
    }
}

pub trait FromWebsocketMessage {
    fn from_message(msg: Message) -> Result<Self, FromWebsocketError> where Self: Sized;
}

#[derive(Debug)]
pub enum FromWebsocketError {
    UnexpectedMessage(Message),
    SerdeError(serde_json::Error)
}
impl From<serde_json::Error> for FromWebsocketError {
    fn from(value: serde_json::Error) -> Self {
        FromWebsocketError::SerdeError(value)
    }
}

impl<T: for<'a> Deserialize<'a>> FromWebsocketMessage for T {
    fn from_message(msg: Message) -> Result<Self, FromWebsocketError> {
        match msg {
            Message::Text(txt) => Ok(serde_json::from_str(&txt)?),
            v => Err(FromWebsocketError::UnexpectedMessage(v))
        }
    }
}