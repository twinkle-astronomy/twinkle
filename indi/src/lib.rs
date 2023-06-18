//! # A general purpose library for interacting with the INDI protocol.
//! The Instrument Neutral Distributed Interface (INDI for short) protocol is
//! an XML-like communicatinos protocol used in the astronomical community
//! to control and monitor astronomical equipment.  For more information on INDI see
//! the project's website [here](https://indilib.org/).
//!
//! The purpose of this crate is to provide a convinent way to interact with devices
//! using the INDI protocol.  Details on the protocol can be found [here](http://docs.indilib.org/protocol/INDI.pdf).
//!
//! ## Quickstart
//! ### Prerequisites
//! To compile this crate, you must first have the libcfitsio library installed.  For debian based linux distros this can be satisfied by running:
//! ```shell
//! $ sudo apt install libcfitsio-dev
//! ```
//!
//! Once that's complete, using you should be able to use cargo to install the crate.
//!
//! ### Simple usage.
//!
//! The simpliest way to use this crate is to open a [TcpStream](std::net::TcpStream) and read/write INDI [commands](crate::serialization::Command).
//! #### Example
//! ```no_run
//! use std::net::TcpStream;
//! use indi::client::ClientConnection;
//!
//! fn main() {
//!     // Connect to local INDI server.
//!     let connection = TcpStream::connect("127.0.0.1:7624").expect("Connecting to INDI server");
//!
//!     // Write command to server instructing it to track all properties.
//!     connection.write(&indi::serialization::GetProperties {
//!         version: indi::INDI_PROTOCOL_VERSION.to_string(),
//!         device: None,
//!         name: None,
//!     })
//!     .expect("Sending GetProperties command");
//!
//!     // Loop through commands recieved from the INDI server
//!     for command in connection.iter().expect("Creating iterator over commands") {
//!         println!("Received from server: {:?}", command);
//!     }
//! }
//! ```
//!
//! ### Using the Client interface
//! The simple usage above has its uses, but if you want to track and modify the state of devices at an INDI server it is recommended to use
//! the [client interface](crate::client::Client).  The client allows you to get [devices](crate::client::device::ActiveDevice),
//! be [notified](crate::client::notify) of changes to those devices, and request [changes](crate::client::device::ActiveDevice::change).
//! #### Example
//! ```no_run
//! use std::time::Duration;
//! use std::net::TcpStream;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create a client with a connection to localhost listening for all device properties.
//!     let client = indi::client::new(
//!         TcpStream::connect("127.0.0.1:7624").expect("Connecting to INDI server"),
//!         None,
//!         None).expect("Initializing connection");
//!
//!     // Get an specific camera device
//!     let camera = client
//!         .get_device::<()>("ZWO CCD ASI294MM Pro")
//!         .await
//!         .expect("Getting camera device");
//!
//!     // Setting the 'CONNECTION' parameter to `on` to ensure the indi device is connected.
//!     camera
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
//!     // Configuring a varienty of the camera's properties at the same time.
//!     tokio::try_join!(
//!         camera.change("CCD_CAPTURE_FORMAT", vec![("ASI_IMG_RAW16", true)]),
//!         camera.change("CCD_TRANSFER_FORMAT", vec![("FORMAT_FITS", true)]),
//!         camera.change("CCD_CONTROLS", vec![("Offset", 10.0), ("Gain", 240.0)]),
//!         camera.change("FITS_HEADER", vec![("FITS_OBJECT", "")]),
//!         camera.change("CCD_BINNING", vec![("HOR_BIN", 2.0), ("VER_BIN", 2.0)]),
//!         camera.change("CCD_FRAME_TYPE", vec![("FRAME_FLAT", true)]),
//!         )
//!         .expect("Configuring camera");
//!
//!     // Capture a 5 second exposure from the camera
//!     let fits = camera.capture_image(Duration::from_secs(5)).await.expect("Capturing image");
//!
//!     // Save the fits file to disk.
//!     fits.save("flat.fits").expect("Saving image");
//! }

use quick_xml::events;
use quick_xml::events::attributes::AttrError;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::BytesText;
use quick_xml::events::Event;
use quick_xml::Result as XmlResult;
use quick_xml::Writer;
use serde::Deserialize;

use std::borrow::Cow;

use std::num;
use std::num::Wrapping;

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

#[derive(Debug, PartialEq, Clone, Copy, Deserialize)]
pub enum PropertyState {
    Idle,
    Ok,
    Busy,
    Alert,
}

#[derive(Debug, PartialEq, Clone, Copy, Deserialize)]
pub enum SwitchState {
    On,
    Off,
}

#[derive(Debug, PartialEq, Clone, Copy, Deserialize)]
pub enum SwitchRule {
    OneOfMany,
    AtMostOne,
    AnyOfMany,
}

#[derive(Debug, PartialEq, Clone, Copy, Deserialize)]
pub enum PropertyPerm {
    #[serde(rename = "ro")]
    RO,
    #[serde(rename = "wo")]
    WO,
    #[serde(rename = "rw")]
    RW,
}

#[derive(Debug, PartialEq, Clone, Copy, Deserialize)]
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

#[derive(Debug, PartialEq, Clone)]
pub struct SwitchVector {
    pub gen: core::num::Wrapping<usize>,
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

#[derive(Debug, PartialEq, Clone)]
pub struct NumberVector {
    pub gen: core::num::Wrapping<usize>,
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
    label: Option<String>,
    value: PropertyState,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LightVector {
    pub gen: core::num::Wrapping<usize>,
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
    pub gen: core::num::Wrapping<usize>,
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
    pub gen: core::num::Wrapping<usize>,
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

    pub fn gen(&self) -> core::num::Wrapping<usize> {
        match self {
            Parameter::TextVector(p) => p.gen,
            Parameter::NumberVector(p) => p.gen,
            Parameter::SwitchVector(p) => p.gen,
            Parameter::LightVector(p) => p.gen,
            Parameter::BlobVector(p) => p.gen,
        }
    }

    pub fn gen_mut(&mut self) -> &mut core::num::Wrapping<usize> {
        match self {
            Parameter::TextVector(p) => &mut p.gen,
            Parameter::NumberVector(p) => &mut p.gen,
            Parameter::SwitchVector(p) => &mut p.gen,
            Parameter::LightVector(p) => &mut p.gen,
            Parameter::BlobVector(p) => &mut p.gen,
        }
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

impl Into<SwitchState> for bool {
    fn into(self) -> SwitchState {
        match self {
            true => SwitchState::On,
            false => SwitchState::Off,
        }
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
