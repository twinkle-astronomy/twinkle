use quick_xml::events;
use quick_xml::events::attributes::AttrError;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::BytesText;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};

use std::borrow::Cow;
use std::collections::HashMap;
use std::net::TcpStream;

use std::io::{BufReader, BufWriter, Write};

use std::num;
use std::str;

use chrono::format::ParseError;
use chrono::prelude::*;
use std::str::FromStr;

static INDI_PROTOCOL_VERSION: &str = "1.7";

pub mod deserialize;
pub struct Client {
    connection: TcpStream,
    xml_writer: Writer<BufWriter<TcpStream>>,
}

#[derive(Debug)]
pub enum Command {
    DefTextVector(DefTextVector),
    SetTextVector(SetTextVector),
    DefNumberVector(DefNumberVector),
    SetNumberVector(SetNumberVector),
    DefSwitchVector(DefSwitchVector),
    SetSwitchVector(SetSwitchVector),
    DefLightVector(DefLightVector),
    SetLightVector(SetLightVector),
    DefBlobVector(DefBlobVector),
    SetBlobVector(SetBlobVector),
    Message(Message),
}

#[derive(Debug, PartialEq)]
pub enum PropertyState {
    Idle,
    Ok,
    Busy,
    Alert,
}

#[derive(Debug, PartialEq)]
pub enum SwitchState {
    On,
    Off,
}

#[derive(Debug, PartialEq)]
pub enum SwitchRule {
    OneOfMany,
    AtMostOne,
    AnyOfMany,
}

#[derive(Debug, PartialEq)]
pub enum PropertyPerm {
    RO,
    WO,
    RW,
}

#[derive(Debug, PartialEq)]
pub enum BlobEnable {
    Never,
    Also,
    Only,
}

#[derive(Debug)]
pub struct DefTextVector {
    pub device: String,
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub texts: HashMap<String, DefText>,
}

#[derive(Debug, PartialEq)]
pub struct DefText {
    pub name: String,
    pub label: Option<String>,
    pub value: String,
}

#[derive(Debug)]
pub struct SetTextVector {
    pub device: String,
    pub name: String,
    pub state: PropertyState,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub texts: HashMap<String, OneText>,
}

#[derive(Debug, PartialEq)]
pub struct OneText {
    pub name: String,
    pub value: String,
}

#[derive(Debug)]
pub struct DefNumberVector {
    pub device: String,
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub numbers: HashMap<String, DefNumber>,
}

#[derive(Debug, PartialEq)]
pub struct DefNumber {
    name: String,
    label: Option<String>,
    format: String,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
}

#[derive(Debug)]
pub struct SetNumberVector {
    pub device: String,
    pub name: String,
    pub state: PropertyState,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub numbers: HashMap<String, OneNumber>,
}

#[derive(Debug, PartialEq)]
pub struct OneNumber {
    name: String,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    value: f64,
}
#[derive(Debug)]
pub struct DefSwitchVector {
    pub device: String,
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub rule: SwitchRule,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub switches: HashMap<String, DefSwitch>,
}

#[derive(Debug, PartialEq)]
pub struct DefSwitch {
    name: String,
    label: Option<String>,
    value: SwitchState,
}

#[derive(Debug)]
pub struct SetSwitchVector {
    pub device: String,
    pub name: String,
    pub state: PropertyState,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub switches: HashMap<String, OneSwitch>,
}

#[derive(Debug, PartialEq)]
pub struct OneSwitch {
    name: String,
    value: SwitchState,
}

#[derive(Debug)]
pub struct DefLightVector {
    pub device: String,
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub lights: HashMap<String, DefLight>,
}

#[derive(Debug, PartialEq)]
pub struct DefLight {
    name: String,
    label: Option<String>,
    value: PropertyState,
}

#[derive(Debug)]
pub struct SetLightVector {
    pub device: String,
    pub name: String,
    pub state: PropertyState,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub lights: HashMap<String, OneLight>,
}

#[derive(Debug, PartialEq)]
pub struct OneLight {
    name: String,
    value: PropertyState,
}

#[derive(Debug)]
pub struct DefBlobVector {
    pub device: String,
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub blobs: HashMap<String, DefBlob>,
}

#[derive(Debug, PartialEq)]
pub struct DefBlob {
    name: String,
    label: Option<String>,
}

#[derive(Debug)]
pub struct SetBlobVector {
    pub device: String,
    pub name: String,
    pub state: PropertyState,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub blobs: HashMap<String, OneBlob>,
}

#[derive(Debug, PartialEq)]
pub struct OneBlob {
    name: String,
    size: u64,
    enclen: Option<u64>,
    format: String,
    value: Vec<u8>,
}

#[derive(Debug, PartialEq)]
pub struct Message {
    pub device: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,
}

#[derive(Debug)]
pub enum DeError {
    XmlError(quick_xml::Error),
    DecodeUtf8(str::Utf8Error),
    DecodeLatin(Cow<'static, str>),
    ParseIntError(num::ParseIntError),
    ParseFloatError(num::ParseFloatError),
    ParseDateTimeError(ParseError),
    MissingAttr(&'static str),
    BadAttr(AttrError),
    UnexpectedAttr(String),
    UnexpectedEvent(String),
    UnexpectedTag(String),
}

impl From<quick_xml::Error> for DeError {
    fn from(err: quick_xml::Error) -> Self {
        DeError::XmlError(err)
    }
}
impl From<str::Utf8Error> for DeError {
    fn from(err: str::Utf8Error) -> Self {
        DeError::DecodeUtf8(err)
    }
}
impl From<Cow<'static, str>> for DeError {
    fn from(err: Cow<'static, str>) -> Self {
        DeError::DecodeLatin(err)
    }
}
impl From<num::ParseIntError> for DeError {
    fn from(err: num::ParseIntError) -> Self {
        DeError::ParseIntError(err)
    }
}
impl From<num::ParseFloatError> for DeError {
    fn from(err: num::ParseFloatError) -> Self {
        DeError::ParseFloatError(err)
    }
}
impl From<ParseError> for DeError {
    fn from(err: ParseError) -> Self {
        DeError::ParseDateTimeError(err)
    }
}
impl From<AttrError> for DeError {
    fn from(err: AttrError) -> Self {
        DeError::BadAttr(err)
    }
}

impl Client {
    pub fn new(addr: &str) -> std::io::Result<Client> {
        let connection = TcpStream::connect(addr)?;
        let xml_writer = Writer::new_with_indent(BufWriter::new(connection.try_clone()?), b' ', 2);

        Ok(Client {
            connection,
            xml_writer,
        })
    }

    pub fn command_iter(
        &self,
    ) -> Result<deserialize::CommandIter<BufReader<TcpStream>>, std::io::Error> {
        let mut xml_reader = Reader::from_reader(BufReader::new(self.connection.try_clone()?));
        xml_reader.trim_text(true);
        xml_reader.expand_empty_elements(true);
        Ok(deserialize::CommandIter::new(xml_reader))
    }

    pub fn query_devices(&mut self) {
        self.xml_writer
            .create_element("getProperties")
            .with_attribute(("version", INDI_PROTOCOL_VERSION))
            .write_empty()
            .unwrap();
        self.xml_writer.write_indent().unwrap();
        self.xml_writer.inner().flush().unwrap();
    }
}
