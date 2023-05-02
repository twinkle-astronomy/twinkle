pub mod number_vector;
use std::sync::PoisonError;

pub use number_vector::DefNumberIter;
pub use number_vector::NewNumberIter;
pub use number_vector::SetNumberIter;

pub mod text_vector;
pub use text_vector::DefTextIter;
pub use text_vector::NewTextIter;
pub use text_vector::SetTextIter;

pub mod switch_vector;
pub use switch_vector::DefSwitchIter;
pub use switch_vector::NewSwitchIter;
pub use switch_vector::SetSwitchIter;

pub mod light_vector;
pub use light_vector::DefLightIter;
pub use light_vector::SetLightIter;

pub mod blob_vector;
pub use blob_vector::DefBlobIter;
pub use blob_vector::SetBlobIter;

pub mod message;
pub use message::MessageIter;

pub mod del_property;
pub use del_property::DelPropertyIter;

pub mod get_properties;
use super::*;
pub use get_properties::GetPropertiesIter;

use quick_xml::name::QName;
use quick_xml::Result as XmlResult;
use quick_xml::{Reader, Writer};

#[derive(Debug)]
pub enum Command {
    // Commands from Device to Connections
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
    EnableBlob(EnableBlob),

    // Commands from Connection to Device
    GetProperties(GetProperties),
}

impl Command {
    pub fn device_name(&self) -> Option<&String> {
        match self {
            Command::DefTextVector(c) => Some(&c.device),
            Command::SetTextVector(c) => Some(&c.device),
            Command::NewTextVector(c) => Some(&c.device),
            Command::DefNumberVector(c) => Some(&c.device),
            Command::SetNumberVector(c) => Some(&c.device),
            Command::NewNumberVector(c) => Some(&c.device),
            Command::DefSwitchVector(c) => Some(&c.device),
            Command::SetSwitchVector(c) => Some(&c.device),
            Command::NewSwitchVector(c) => Some(&c.device),
            Command::DefLightVector(c) => Some(&c.device),
            Command::SetLightVector(c) => Some(&c.device),
            Command::DefBlobVector(c) => Some(&c.device),
            Command::SetBlobVector(c) => Some(&c.device),
            Command::Message(c) => match &c.device {
                Some(device) => Some(device),
                None => None,
            },
            Command::DelProperty(c) => Some(&c.device),
            Command::GetProperties(c) => match &c.device {
                Some(device) => Some(device),
                None => None,
            },
            Command::EnableBlob(c) => Some(&c.device),
        }
    }
}

impl XmlSerialization for Command {
    fn write<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        match self {
            Command::NewTextVector(c) => c.write(xml_writer),
            Command::NewNumberVector(c) => c.write(xml_writer),
            Command::NewSwitchVector(c) => c.write(xml_writer),
            Command::EnableBlob(c) => c.write(xml_writer),

            _ => todo!(),
        }
    }
}

pub trait ToCommand<T> {
    fn to_command(self, device_name: String, param_name: String) -> Command;
}

impl<I: Into<SwitchState> + Copy> ToCommand<Vec<(&str, I)>> for Vec<(&str, I)> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewSwitchVector(NewSwitchVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            switches: self
                .iter()
                .map(|x| OneSwitch {
                    name: String::from(x.0),
                    value: x.1.into(),
                })
                .collect(),
        })
    }
}

impl ToCommand<Vec<OneSwitch>> for Vec<OneSwitch> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewSwitchVector(NewSwitchVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            switches: self,
        })
    }
}

impl ToCommand<Vec<(&str, f64)>> for Vec<(&str, f64)> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewNumberVector(NewNumberVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            numbers: self
                .iter()
                .map(|x| OneNumber {
                    name: String::from(x.0),
                    value: x.1,
                })
                .collect(),
        })
    }
}
impl ToCommand<Vec<OneNumber>> for Vec<OneNumber> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewNumberVector(NewNumberVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            numbers: self,
        })
    }
}

impl ToCommand<Vec<(&str, &str)>> for Vec<(&str, &str)> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewTextVector(NewTextVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            texts: self
                .iter()
                .map(|x| OneText {
                    name: String::from(x.0),
                    value: String::from(x.1),
                })
                .collect(),
        })
    }
}
impl ToCommand<Vec<OneText>> for Vec<OneText> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewTextVector(NewTextVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            texts: self,
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum UpdateError {
    ParameterMissing(String),
    ParameterTypeMismatch(String),
    PoisonError,
}

impl<T> From<PoisonError<T>> for UpdateError {
    fn from(_: PoisonError<T>) -> Self {
        UpdateError::PoisonError
    }
}

pub enum Action {
    Define,
    Update,
    Delete,
}

pub trait CommandtoParam {
    fn get_name(&self) -> &String;
    fn get_group(&self) -> &Option<String>;
    fn to_param(self, gen: Wrapping<usize>) -> Parameter;
}

pub trait CommandToUpdate {
    fn get_name(&self) -> &String;
    fn update_param(self, param: &mut Parameter) -> Result<String, UpdateError>;
}

pub enum ClientErrors {
    DeError(DeError),
    UpdateError(UpdateError),
}

impl From<DeError> for ClientErrors {
    fn from(err: DeError) -> Self {
        ClientErrors::DeError(err)
    }
}
impl From<UpdateError> for ClientErrors {
    fn from(err: UpdateError) -> Self {
        ClientErrors::UpdateError(err)
    }
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

#[derive(Debug, PartialEq, Clone)]
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
    pub name: String,
    pub label: Option<String>,
    pub format: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: f64,
}

#[derive(Debug)]
pub struct SetNumberVector {
    pub device: String,
    pub name: String,
    pub state: PropertyState,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: Option<String>,

    pub numbers: Vec<SetOneNumber>,
}

#[derive(Debug, PartialEq)]
pub struct SetOneNumber {
    pub name: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub value: f64,
}

#[derive(Debug)]
pub struct NewNumberVector {
    pub device: String,
    pub name: String,
    pub timestamp: Option<DateTime<Utc>>,

    pub numbers: Vec<OneNumber>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct OneNumber {
    pub name: String,
    pub value: f64,
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
    pub name: String,
    pub label: Option<String>,
    pub value: SwitchState,
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

#[derive(Debug, PartialEq, Clone)]
pub struct OneSwitch {
    pub name: String,
    pub value: SwitchState,
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
    pub name: String,
    pub size: u64,
    pub enclen: Option<u64>,
    pub format: String,
    pub value: Vec<u8>,
}

#[derive(Debug, PartialEq)]
pub struct EnableBlob {
    pub device: String,
    pub name: Option<String>,

    pub enabled: BlobEnable,
}

#[derive(Debug, PartialEq, Clone)]
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

pub trait XmlSerialization {
    fn write<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>>;
}

#[derive(Debug)]
pub enum DeError {
    XmlError(quick_xml::Error),
    IoError(std::io::Error),
    DecodeUtf8(str::Utf8Error),
    DecodeLatin(Cow<'static, str>),
    ParseIntError(num::ParseIntError),
    ParseFloatError(num::ParseFloatError),
    ParseSexagesimalError(String),
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

impl From<std::io::Error> for DeError {
    fn from(err: std::io::Error) -> Self {
        DeError::IoError(err)
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

impl<'a> SwitchRule {
    fn try_from<T: std::io::BufRead>(
        value: Attribute<'a>,
        xml_reader: &Reader<T>,
    ) -> Result<Self, DeError> {
        match value.decode_and_unescape_value(xml_reader)? {
            Cow::Borrowed("OneOfMany") => Ok(SwitchRule::OneOfMany),
            Cow::Borrowed("AtMostOne") => Ok(SwitchRule::AtMostOne),
            Cow::Borrowed("AnyOfMany") => Ok(SwitchRule::AnyOfMany),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}

impl<'a> PropertyState {
    fn try_from<T: std::io::BufRead>(
        value: Attribute<'a>,
        xml_reader: &Reader<T>,
    ) -> Result<Self, DeError> {
        match value.decode_and_unescape_value(xml_reader)? {
            Cow::Borrowed("Idle") => Ok(PropertyState::Idle),
            Cow::Borrowed("Ok") => Ok(PropertyState::Ok),
            Cow::Borrowed("Busy") => Ok(PropertyState::Busy),
            Cow::Borrowed("Alert") => Ok(PropertyState::Alert),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }

    fn try_from_event(value: BytesText<'a>) -> Result<Self, DeError> {
        match value.unescape()? {
            Cow::Borrowed("Idle") => Ok(PropertyState::Idle),
            Cow::Borrowed("Ok") => Ok(PropertyState::Ok),
            Cow::Borrowed("Busy") => Ok(PropertyState::Busy),
            Cow::Borrowed("Alert") => Ok(PropertyState::Alert),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}

impl<'a> SwitchState {
    fn try_from_event(value: BytesText<'a>) -> Result<Self, DeError> {
        match value.unescape()? {
            Cow::Borrowed("On") => Ok(SwitchState::On),
            Cow::Borrowed("Off") => Ok(SwitchState::Off),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}
impl<'a> PropertyPerm {
    fn try_from<T: std::io::BufRead>(
        value: Attribute<'a>,
        xml_reader: &Reader<T>,
    ) -> Result<Self, DeError> {
        match value.decode_and_unescape_value(xml_reader)? {
            Cow::Borrowed("ro") => Ok(PropertyPerm::RO),
            Cow::Borrowed("wo") => Ok(PropertyPerm::WO),
            Cow::Borrowed("rw") => Ok(PropertyPerm::RW),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}

pub struct CommandIter<T: std::io::BufRead> {
    xml_reader: Reader<T>,
    buf: Vec<u8>,
}

impl<T: std::io::BufRead> Iterator for CommandIter<T> {
    type Item = Result<Command, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_command() {
            Ok(Some(command)) => {
                return Some(Ok(command));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}

impl<T: std::io::BufRead> CommandIter<T> {
    pub fn new(xml_reader: Reader<T>) -> CommandIter<T> {
        let buf = Vec::new();
        CommandIter { xml_reader, buf }
    }

    pub fn buffer_position(&self) -> usize {
        self.xml_reader.buffer_position()
    }

    fn next_command(&mut self) -> Result<Option<Command>, DeError> {
        self.buf.truncate(0);
        let event = self.xml_reader.read_event_into(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                QName(b"defTextVector") => {
                    let mut text_vector = DefTextIter::text_vector(&self.xml_reader, &e)?;

                    for text in DefTextIter::new(self) {
                        let text = text?;
                        text_vector.texts.push(text);
                    }

                    Ok(Some(Command::DefTextVector(text_vector)))
                }
                QName(b"setTextVector") => {
                    let mut text_vector = SetTextIter::text_vector(&self.xml_reader, &e)?;

                    for text in SetTextIter::new(self) {
                        let text = text?;
                        text_vector.texts.push(text);
                    }

                    Ok(Some(Command::SetTextVector(text_vector)))
                }
                QName(b"newTextVector") => {
                    let mut text_vector = NewTextIter::text_vector(&self.xml_reader, &e)?;

                    for text in NewTextIter::new(self) {
                        let text = text?;
                        text_vector.texts.push(text);
                    }

                    Ok(Some(Command::NewTextVector(text_vector)))
                }
                QName(b"defNumberVector") => {
                    let mut number_vector = DefNumberIter::number_vector(&self.xml_reader, &e)?;

                    for number in DefNumberIter::new(self) {
                        let number = number?;
                        number_vector.numbers.push(number);
                    }

                    Ok(Some(Command::DefNumberVector(number_vector)))
                }
                QName(b"setNumberVector") => {
                    let mut number_vector = SetNumberIter::number_vector(&self.xml_reader, &e)?;

                    for number in SetNumberIter::new(self) {
                        let number = number?;
                        number_vector.numbers.push(number);
                    }

                    Ok(Some(Command::SetNumberVector(number_vector)))
                }
                QName(b"newNumberVector") => {
                    let mut number_vector = NewNumberIter::number_vector(&self.xml_reader, &e)?;

                    for number in NewNumberIter::new(self) {
                        let number = number?;
                        number_vector.numbers.push(number);
                    }

                    Ok(Some(Command::NewNumberVector(number_vector)))
                }
                QName(b"defSwitchVector") => {
                    let mut switch_vector = DefSwitchIter::switch_vector(&self.xml_reader, &e)?;

                    for switch in DefSwitchIter::new(self) {
                        let switch = switch?;
                        switch_vector.switches.push(switch);
                    }

                    Ok(Some(Command::DefSwitchVector(switch_vector)))
                }
                QName(b"setSwitchVector") => {
                    let mut switch_vector = SetSwitchIter::switch_vector(&self.xml_reader, &e)?;

                    for switch in SetSwitchIter::new(self) {
                        let switch = switch?;
                        switch_vector.switches.push(switch);
                    }

                    Ok(Some(Command::SetSwitchVector(switch_vector)))
                }
                QName(b"newSwitchVector") => {
                    let mut switch_vector = NewSwitchIter::switch_vector(&self.xml_reader, &e)?;

                    for switch in NewSwitchIter::new(self) {
                        let switch = switch?;
                        switch_vector.switches.push(switch);
                    }

                    Ok(Some(Command::NewSwitchVector(switch_vector)))
                }
                QName(b"defLightVector") => {
                    let mut light_vector = DefLightIter::light_vector(&self.xml_reader, &e)?;

                    for light in DefLightIter::new(self) {
                        let light = light?;
                        light_vector.lights.push(light);
                    }

                    Ok(Some(Command::DefLightVector(light_vector)))
                }
                QName(b"setLightVector") => {
                    let mut light_vector = SetLightIter::light_vector(&self.xml_reader, &e)?;

                    for light in SetLightIter::new(self) {
                        let light = light?;
                        light_vector.lights.push(light);
                    }

                    Ok(Some(Command::SetLightVector(light_vector)))
                }
                QName(b"defBLOBVector") => {
                    let mut blob_vector = DefBlobIter::blob_vector(&self.xml_reader, &e)?;

                    for blob in DefBlobIter::new(self) {
                        let blob = blob?;
                        blob_vector.blobs.push(blob);
                    }

                    Ok(Some(Command::DefBlobVector(blob_vector)))
                }
                QName(b"setBLOBVector") => {
                    let mut blob_vector = SetBlobIter::blob_vector(&self.xml_reader, &e)?;

                    for blob in SetBlobIter::new(self) {
                        let blob = blob?;
                        blob_vector.blobs.push(blob);
                    }

                    Ok(Some(Command::SetBlobVector(blob_vector)))
                }
                QName(b"message") => {
                    let message = MessageIter::message(&self.xml_reader, &e)?;
                    for _ in MessageIter::new(self) {}

                    Ok(Some(Command::Message(message)))
                }
                QName(b"delProperty") => {
                    let message = DelPropertyIter::del_property(&self.xml_reader, &e)?;
                    for _ in DelPropertyIter::new(self) {}

                    Ok(Some(Command::DelProperty(message)))
                }

                QName(b"getProperties") => {
                    let get_properties = GetPropertiesIter::get_properties(&self.xml_reader, &e)?;
                    for _ in GetPropertiesIter::new(self) {}

                    Ok(Some(Command::GetProperties(get_properties)))
                }
                tag => Err(DeError::UnexpectedTag(
                    str::from_utf8(tag.into_inner())?.to_string(),
                )),
            },
            Event::End(tag) => {
                println!("Unexpected end: {}", tag.escape_ascii().to_string());
                Err(DeError::UnexpectedEvent(format!("{:?}", tag)))
            }
            Event::Eof => Ok(None),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}
