use quick_xml::events;
use quick_xml::events::attributes::AttrError;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use quick_xml::events::attributes::Attribute;
use quick_xml::events::BytesText;

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
    pub devices: HashMap<String, Device>,
}

#[derive(Debug)]
pub enum Command {
    DefParameter(Parameter),
}

#[derive(Debug)]
pub struct Device {
    pub name: String,

    pub parameters: HashMap<String, Parameter>,
}

#[derive(Debug)]
pub enum Parameter {
    Text(TextVector),
    Number(NumberVector),
    Switch(SwitchVector),
}

#[derive(Debug, PartialEq)]
pub enum PropertyState {
    Idle,
    Ok,
    Busy,
    Alert
}

impl<'a> TryFrom<Attribute<'a>> for PropertyState {
    type Error = DeError;

    fn try_from(value: Attribute<'a>) -> Result<Self, Self::Error> {
        match value.unescaped_value()? {
            Cow::Borrowed(b"Idle") => Ok(PropertyState::Idle),
            Cow::Borrowed(b"Ok") => Ok(PropertyState::Ok),
            Cow::Borrowed(b"Busy") => Ok(PropertyState::Busy),
            Cow::Borrowed(b"Alert") => Ok(PropertyState::Alert),
            _ => return Err(DeError::UnexpectedEvent())
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SwitchState {
    On,
    Off
}

impl<'a> TryFrom<BytesText<'a>> for SwitchState {
    type Error = DeError;

    fn try_from(value: BytesText<'a>) -> Result<Self, Self::Error> {
        match value.unescaped()? {
            Cow::Borrowed(b"On") => Ok(SwitchState::On),
            Cow::Borrowed(b"Off") => Ok(SwitchState::Off),
            _ => return Err(DeError::UnexpectedEvent())
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SwitchRule {
    OneOfMany,
    AtMostOne,
    AnyOfMany
}

impl<'a> TryFrom<Attribute<'a>> for SwitchRule {
    type Error = DeError;

    fn try_from(value: Attribute<'a>) -> Result<Self, Self::Error> {
        match value.unescaped_value()? {
            Cow::Borrowed(b"OneOfMany") => Ok(SwitchRule::OneOfMany),
            Cow::Borrowed(b"AtMostOne") => Ok(SwitchRule::AtMostOne),
            Cow::Borrowed(b"AnyOfMany") => Ok(SwitchRule::AnyOfMany),
            _ => return Err(DeError::UnexpectedEvent())
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum PropertyPerm {
    RO,
    WO,
    RW
}

impl<'a> TryFrom<Attribute<'a>> for PropertyPerm {
    type Error = DeError;

    fn try_from(value: Attribute<'a>) -> Result<Self, Self::Error> {
        match value.unescaped_value()? {
            Cow::Borrowed(b"ro") => Ok(PropertyPerm::RO),
            Cow::Borrowed(b"wo") => Ok(PropertyPerm::WO),
            Cow::Borrowed(b"rw") => Ok(PropertyPerm::RW),
            _ => return Err(DeError::UnexpectedEvent())
        }
    }
}


#[derive(Debug, PartialEq)]
pub enum BlobEnable {
    Never,
    Also,
    Only
}






#[derive(Debug)]
pub struct TextVector {
    pub device: String,
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub texts: HashMap<String, Text>,
}

#[derive(Debug, PartialEq)]
pub struct Text {
    name: String,
    label: Option<String>,
    value: String,
}

#[derive(Debug)]
pub struct NumberVector {
    pub device: String,
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub numbers: HashMap<String, Number>,
}

#[derive(Debug, PartialEq)]
pub struct Number {
    name: String,
    label: Option<String>,
    format: String,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
}


#[derive(Debug)]
pub struct SwitchVector {
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

    pub switches: HashMap<String, Switch>,
}

#[derive(Debug, PartialEq)]
pub struct Switch {
    name: String,
    label: Option<String>,
    value: SwitchState,
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
    UnexpectedEvent(),
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

        let devices = HashMap::new();
        Ok(Client {
            connection,
            xml_writer,
            devices,
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
