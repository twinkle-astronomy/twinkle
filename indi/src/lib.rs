use quick_xml::events;
use quick_xml::events::attributes::AttrError;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::BytesText;
use quick_xml::events::Event;
use quick_xml::Result as XmlResult;
use quick_xml::{Reader, Writer};

use std::borrow::Cow;
use std::net::TcpStream;

use std::io::{BufReader, BufWriter};

use std::num;
use std::str;

use chrono::format::ParseError;
use chrono::prelude::*;
use std::io::Write;
use std::str::FromStr;

pub static INDI_PROTOCOL_VERSION: &str = "1.7";

pub mod serialization;
use serialization::XmlSerialization;

#[derive(Debug)]
pub enum Command {
    // Commands from Device to Clients
    DefTextVector(DefTextVector),
    SetTextVector(SetTextVector),
    NewTextVector(NewTextVector),
    DefNumberVector(DefNumberVector),
    SetNumberVector(SetNumberVector),
    NewNumberVector(NewNumberVector),
    DefSwitchVector(DefSwitchVector),
    SetSwitchVector(SetSwitchVector),
    NewSwitchVector(NewSwitchVector),
    DefLightVector(DefLightVector),
    SetLightVector(SetLightVector),
    DefBlobVector(DefBlobVector),
    SetBlobVector(SetBlobVector),
    Message(Message),
    DelProperty(DelProperty),

    // Commands from Client to Device
    GetProperties(GetProperties),
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

    pub texts: Vec<DefText>,
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

    pub texts: Vec<OneText>,
}

#[derive(Debug)]
pub struct NewTextVector {
    pub device: String,
    pub name: String,
    pub timestamp: Option<DateTime<Utc>>,

    pub texts: Vec<OneText>,
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

    pub numbers: Vec<DefNumber>,
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

    pub numbers: Vec<OneNumber>,
}

#[derive(Debug)]
pub struct NewNumberVector {
    pub device: String,
    pub name: String,
    pub timestamp: Option<DateTime<Utc>>,

    pub numbers: Vec<OneNumber>,
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

    pub switches: Vec<DefSwitch>,
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

    pub switches: Vec<OneSwitch>,
}
#[derive(Debug)]
pub struct NewSwitchVector {
    pub device: String,
    pub name: String,
    pub timestamp: Option<DateTime<Utc>>,

    pub switches: Vec<OneSwitch>,
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

    pub lights: Vec<DefLight>,
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

    pub lights: Vec<OneLight>,
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

    pub blobs: Vec<DefBlob>,
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

    pub blobs: Vec<OneBlob>,
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
pub struct EnableBlob {
    pub device: String,
    pub name: Option<String>,

    pub enabled: BlobEnable,
}

#[derive(Debug, PartialEq)]
pub struct Message {
    pub device: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct DelProperty {
    pub device: String,
    pub name: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct GetProperties {
    pub version: String,
    pub device: Option<String>,
    pub name: Option<String>,
}

pub struct Client {
    connection: TcpStream,
    xml_writer: Writer<BufWriter<TcpStream>>,
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
    ) -> Result<serialization::CommandIter<BufReader<TcpStream>>, std::io::Error> {
        let mut xml_reader = Reader::from_reader(BufReader::new(self.connection.try_clone()?));
        xml_reader.trim_text(true);
        xml_reader.expand_empty_elements(true);
        Ok(serialization::CommandIter::new(xml_reader))
    }

    pub fn send<T: XmlSerialization>(&mut self, command: &T) -> Result<(), quick_xml::Error> {
        command.send(&mut self.xml_writer)?;
        self.xml_writer.inner().flush()?;
        Ok(())
    }
}
