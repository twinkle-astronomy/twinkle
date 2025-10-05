use serde::{Deserialize, Serialize};

pub mod analysis;
pub mod capture;
pub mod fits;
pub mod flats;
pub mod indi;
#[cfg(feature = "wasm")]
pub use tokio_tungstenite_wasm::Message;

#[cfg(feature = "native")]
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
    fn from_message(msg: Message) -> Result<Self, FromWebsocketError>
    where
        Self: Sized;
}

#[derive(Debug)]
pub enum FromWebsocketError {
    UnexpectedMessage(Message),
    SerdeError(serde_json::Error),
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
            v => Err(FromWebsocketError::UnexpectedMessage(v)),
        }
    }
}
