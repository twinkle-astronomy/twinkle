//! # A general purpose library for interacting with the INDI protocol.
//! The Instrument Neutral Distributed Interface (INDI for short) protocol is
//! an XML-like communicatinos protocol used in the astronomical community
//! to control and monitor astronomical equipment.  For more information on INDI see
//! the project's website [here](https://indilib.org/).
//!
//! The purpose of this crate is to provide a convinent way to interact with devices
//! using the INDI protocol.  Details on the protocol can be found [here](http://docs.indilib.org/protocol/INDI.pdf).
//!
//! ### Simple usage.
//!
//! The simpliest way to use this crate is to open a [TcpStream](std::net::TcpStream) and read/write INDI [commands](crate::serialization::Command).
//! #### Example
//! ```no_run
//! use tokio::net::TcpStream;
//! use tokio_stream::{Stream, StreamExt};
//! use crate::indi::client::{AsyncClientConnection,AsyncReadConnection,AsyncWriteConnection};
//! #[tokio::main]
//! async fn main() {
//!     // Connect to local INDI server.
//!     let connection = TcpStream::connect("127.0.0.1:7624").await.expect("Connecting to INDI server");
//!     let (mut writer, mut reader) = connection.to_indi();
//!
//!     // Write command to server instructing it to track all properties.
//!     writer.write(indi::serialization::Command::GetProperties(indi::serialization::GetProperties {
//!         version: indi::INDI_PROTOCOL_VERSION.to_string(),
//!         device: None,
//!         name: None,
//!     }))
//!     .await
//!     .expect("Sending GetProperties command");
//!
//!     // Loop through commands recieved from the INDI server
//!     loop {
//!         let command = match reader.read().await {
//!             Some(command) => command,
//!             None => break,
//!         }.unwrap();
//!         println!("Received from server: {:?}", command);
//!     }
//! }
//! ```
//!
//! ### Using the Client interface
//! The simple usage above has its uses, but if you want to track and modify the state of devices at an INDI server it is recommended to use
//! the [client interface](crate::client::Client).  The client allows you to get [devices](crate::client::active_device::ActiveDevice),
//! be [notified](crate::client::notify) of changes to those devices, and request [changes](crate::client::active_device::ActiveDevice::change).
//! #### Example
//! ```no_run
//! use std::time::Duration;
//! use tokio::net::TcpStream;
//! use twinkle_client::task::Task;
//! use twinkle_client::task::Status;
//! use std::ops::Deref;
//! use indi::serialization::Sexagesimal;
//! use tokio_stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create a client with a connection to localhost listening for all device properties.
//!     let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
//!     let client = indi::client::Client::new(Some(tx));
//!     let connection = TcpStream::connect("127.0.0.1:7624").await.unwrap();
//!     let _client_task: tokio::task::JoinHandle<()> = tokio::task::spawn(indi::client::start(client.get_devices().clone(), rx, connection));
//!
//!     // Get an specific camera device
//!     let camera = client
//!         .get_device("ZWO CCD ASI294MM Pro")
//!         .await
//!         .expect("Getting camera device");
//!
//!     // Setting the 'CONNECTION' parameter to `on` to ensure the indi device is connected.
//!     let _ = camera
//!         .change("CONNECTION", vec![("CONNECT", true)])
//!         .await
//!         .expect("Connecting to camera");
//!
//!     // Enabling blob transport for the camera.
//!     camera
//!         .enable_blob(Some("CCD1"), indi::BlobEnable::Also)
//!         .await
//!         .expect("Enabling image retrieval");
//!
//!     // Subscribing to changes to the CCD parameter so we can get the next Blob
//!     let ccd = camera.get_parameter("CCD1").await.expect("Getting ccd parameter");
//!     let mut ccd_sub = ccd.changes();
//!
//!     // Configuring a varienty of the camera's properties at the same time.
//!     let _ = tokio::try_join!(
//!         camera.change("CCD_CAPTURE_FORMAT", vec![("ASI_IMG_RAW16", true)]),
//!         camera.change("CCD_TRANSFER_FORMAT", vec![("FORMAT_FITS", true)]),
//!         camera.change("CCD_CONTROLS", vec![("Offset", Sexagesimal::from(10.0)), ("Gain", Sexagesimal::from(240.0))]),
//!         camera.change("CCD_BINNING", vec![("HOR_BIN", Sexagesimal::from(2.0)), ("VER_BIN", Sexagesimal::from(2.0))]),
//!         camera.change("CCD_FRAME_TYPE", vec![("FRAME_FLAT", true)]),
//!         )
//!         .expect("Configuring camera");
//!
//!     // Set exposure
//!     let _ = camera.parameter("CCD_EXPOSURE").await.unwrap().change(vec![("CCD_EXPOSURE_VALUE", Sexagesimal::from(5.0))]).await.expect("Setting the exposure");
//!
//!     // Get the image
//!     if let indi::Parameter::BlobVector(blob) =  ccd_sub.next().await.unwrap().unwrap().as_ref() {
//!         let _image = blob.values.get("CCD1").unwrap();
//!     }
//! }
//! ```
//!
//! ### Using the Telescope interface
//! The client interface is fine if you want generic controll of indi devices, but isn't aware of things like "Camera"s, and only maintains one connection
//! per client.  This can be a problem when streaming images, as no device updates can be received while an image is in transit.  To solve these problems
//! you can use the [telescope] module.  It manages separate image and control connections and has type representing various device types.
//! #### Example
//! ```no_run
//!use indi::telescope::Telescope;
//!use indi::telescope::settings::Settings;
//!use tokio::net::TcpStream;
//!use std::time::Duration;
//!
//!#[tokio::main]
//!async fn main() {
//!    let settings = Settings::default();
//!
//!    // Create a Telescope connected to localhost.
//!    let mut telescope = Telescope::new(settings.telescope_config.clone());
//!    telescope.connect_from_settings::<TcpStream>(&settings).await;
//!
//!    // Get an specific camera device
//!    let camera = telescope.get_primary_camera().await.expect("Getting camera");
//!
//!    // Make sure the camera is connected
//!    let _ = camera.connect().await.expect("Connecting to camera");
//!
//!    let capture_format_param = camera.capture_format().await.expect("Getting capture format");
//!    let transfer_format_param = camera.transfer_format().await.expect("Getting transfer format");
//!    let gain_param = camera.gain().await.expect("getting gain");
//!    let binning_param = camera.binning().await.expect("getting binning");
//!    let image_type_param = camera.image_type().await.expect("Getting image type");
//!
//!    // Configuring a variety of the camera's properties at the same time.
//!    tokio::try_join!(
//!        capture_format_param.change(indi::telescope::camera::CaptureFormat::Raw16),
//!        transfer_format_param.change(indi::telescope::camera::TransferFormat::Fits),
//!        gain_param.change(120.0),
//!        binning_param.change(indi::telescope::camera::Binning {hor: 2, ver: 2}),
//!        image_type_param.change(indi::telescope::camera::ImageType::Light),
//!    ).expect("Configuring the camera");
//!
//!    // Capture an immage
//!    let _blob = camera.capture_image(Duration::from_secs(500)).await.expect("Capturing an image");
//!}
//!```

pub use tokio;

use quick_xml::events::attributes::AttrError;
use serde::Deserialize;
use serde::Serialize;

use std::borrow::Cow;

use std::num;

use std::str;
use std::sync::Arc;

use chrono::format::ParseError;
use chrono::prelude::*;
use std::str::FromStr;

use std::collections::HashMap;

pub static INDI_PROTOCOL_VERSION: &str = "1.7";

pub mod serialization;
use serialization::*;

pub mod client;
pub mod telescope;

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum PropertyState {
    Idle,
    Ok,
    Busy,
    Alert,
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum SwitchState {
    On,
    Off,
}

impl From<bool> for SwitchState {
    fn from(value: bool) -> Self {
        match value {
            true => SwitchState::On,
            false => SwitchState::Off,
        }
    }
}
#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum SwitchRule {
    OneOfMany,
    AtMostOne,
    AnyOfMany,
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum PropertyPerm {
    #[serde(rename = "ro")]
    RO,
    #[serde(rename = "wo")]
    WO,
    #[serde(rename = "rw")]
    RW,
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum BlobEnable {
    Never,
    Also,
    Only,
}

pub trait FromParamValue {
    fn values_from(w: &Parameter) -> Result<&Self, TypeError>
    where
        Self: Sized;
}

#[derive(Debug, PartialEq, Clone)]
pub struct Switch {
    pub label: Option<String>,
    pub value: SwitchState,
}

impl Into<SwitchState> for Switch {
    fn into(self) -> SwitchState {
        self.value
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct SwitchVector {
    pub name: String,
    pub group: Option<String>,
    pub label: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub rule: SwitchRule,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Switch>,
}

impl FromParamValue for HashMap<String, Switch> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::SwitchVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Number {
    pub label: Option<String>,
    pub format: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: Sexagesimal,
}

impl Into<Sexagesimal> for Number {
    fn into(self) -> Sexagesimal {
        self.value
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct NumberVector {
    pub name: String,
    pub group: Option<String>,
    pub label: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Number>,
}

impl FromParamValue for HashMap<String, Number> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::NumberVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Light {
    pub label: Option<String>,
    pub value: PropertyState,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LightVector {
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Light>,
}

impl FromParamValue for HashMap<String, Light> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::LightVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Text {
    pub label: Option<String>,
    pub value: String,
}

impl FromParamValue for HashMap<String, Text> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::TextVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TextVector {
    pub name: String,
    pub group: Option<String>,
    pub label: Option<String>,

    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Text>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Blob {
    pub label: Option<String>,
    pub format: Option<String>,
    pub value: Option<Arc<Vec<u8>>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct BlobVector {
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Blob>,
}

impl FromParamValue for HashMap<String, Blob> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::BlobVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Parameter {
    TextVector(TextVector),
    NumberVector(NumberVector),
    SwitchVector(SwitchVector),
    LightVector(LightVector),
    BlobVector(BlobVector),
}

impl Parameter {
    pub fn get_group(&self) -> &Option<String> {
        match self {
            Parameter::TextVector(p) => &p.group,
            Parameter::NumberVector(p) => &p.group,
            Parameter::SwitchVector(p) => &p.group,
            Parameter::LightVector(p) => &p.group,
            Parameter::BlobVector(p) => &p.group,
        }
    }

    pub fn get_name(&self) -> &String {
        match self {
            Parameter::TextVector(p) => &p.name,
            Parameter::NumberVector(p) => &p.name,
            Parameter::SwitchVector(p) => &p.name,
            Parameter::LightVector(p) => &p.name,
            Parameter::BlobVector(p) => &p.name,
        }
    }
    pub fn get_label(&self) -> &Option<String> {
        match self {
            Parameter::TextVector(p) => &p.label,
            Parameter::NumberVector(p) => &p.label,
            Parameter::SwitchVector(p) => &p.label,
            Parameter::LightVector(p) => &p.label,
            Parameter::BlobVector(p) => &p.label,
        }
    }
    pub fn get_label_display(&self) -> &String {
        match self.get_label() {
            Some(label) => label,
            None => self.get_name(),
        }
    }
    pub fn get_state(&self) -> &PropertyState {
        match self {
            Parameter::TextVector(p) => &p.state,
            Parameter::NumberVector(p) => &p.state,
            Parameter::SwitchVector(p) => &p.state,
            Parameter::LightVector(p) => &p.state,
            Parameter::BlobVector(p) => &p.state,
        }
    }
    pub fn get_timeout(&self) -> &Option<u32> {
        match self {
            Parameter::TextVector(p) => &p.timeout,
            Parameter::NumberVector(p) => &p.timeout,
            Parameter::SwitchVector(p) => &p.timeout,
            Parameter::LightVector(_) => &None,
            Parameter::BlobVector(p) => &p.timeout,
        }
    }

    pub fn get_values<T: FromParamValue>(&self) -> Result<&T, TypeError> {
        T::values_from(self)
    }
}

#[derive(Debug)]
pub enum TypeError {
    TypeMismatch,
}
pub trait TryEq<T> {
    fn try_eq(&self, other: &T) -> Result<bool, TypeError>;
}

impl TryEq<Parameter> for Vec<OneSwitch> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Switch>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.value) == current_values.get(&other_value.name).map(|x| x.value)
        }))
    }
}

impl<I: Into<SwitchState> + Copy> TryEq<Parameter> for Vec<(&str, I)> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Switch>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.1.into()) == current_values.get(other_value.0).map(|x| x.value)
        }))
    }
}

impl TryEq<Parameter> for Vec<(&str, f64)> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Number>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.1) == current_values.get(other_value.0).map(|x| x.value.into())
        }))
    }
}

impl TryEq<Parameter> for Vec<(&str, Sexagesimal)> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Number>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.1) == current_values.get(other_value.0).map(|x| x.value.into())
        }))
    }
}

impl TryEq<Parameter> for Vec<OneNumber> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Number>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.value) == current_values.get(&other_value.name).map(|x| x.value)
        }))
    }
}

impl TryEq<Parameter> for Vec<(&str, &str)> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Text>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.1) == current_values.get(other_value.0).map(|x| x.value.as_str())
        }))
    }
}

impl TryEq<Parameter> for Vec<OneText> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Text>>()?;

        Ok(self.iter().all(|other_value| {
            Some(&other_value.value) == current_values.get(&other_value.name).map(|x| &x.value)
        }))
    }
}
