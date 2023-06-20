pub mod number_vector;
use std::io::BufRead;
use std::ops::Deref;
use std::sync::PoisonError;

pub mod text_vector;
use quick_xml::de::{IoReader, XmlRead};

pub mod blob_vector;
pub mod del_property;
pub mod get_properties;
pub mod light_vector;
pub mod message;
pub mod switch_vector;
use super::*;

use serde::Deserialize;

use quick_xml::Result as XmlResult;
use quick_xml::Writer;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Timestamp(pub DateTime<Utc>);

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Timestamp, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;

        let datetime = DateTime::from_str(&format!("{}Z", s)).unwrap();
        Ok(Timestamp(datetime))
    }
}

impl Deref for Timestamp {
    type Target = DateTime<Utc>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(value: DateTime<Utc>) -> Self {
        Timestamp(value)
    }
}

impl Timestamp {
    pub fn into_inner(self) -> DateTime<Utc> {
        self.0
    }
}

#[derive(Debug, Deserialize)]
pub enum Command {
    // Commands from Device to Connections
    #[serde(rename = "defTextVector")]
    DefTextVector(DefTextVector),
    #[serde(rename = "setTextVector")]
    SetTextVector(SetTextVector),
    #[serde(rename = "newTextVector")]
    NewTextVector(NewTextVector),
    #[serde(rename = "defNumberVector")]
    DefNumberVector(DefNumberVector),
    #[serde(rename = "setNumberVector")]
    SetNumberVector(SetNumberVector),
    #[serde(rename = "newNumberVector")]
    NewNumberVector(NewNumberVector),
    #[serde(rename = "defSwitchVector")]
    DefSwitchVector(DefSwitchVector),
    #[serde(rename = "setSwitchVector")]
    SetSwitchVector(SetSwitchVector),
    #[serde(rename = "newSwitchVector")]
    NewSwitchVector(NewSwitchVector),
    #[serde(rename = "defLightVector")]
    DefLightVector(DefLightVector),
    #[serde(rename = "setLightVector")]
    SetLightVector(SetLightVector),
    #[serde(rename = "defBLOBVector")]
    DefBlobVector(DefBlobVector),
    #[serde(rename = "setBLOBVector")]
    SetBlobVector(SetBlobVector),
    #[serde(rename = "message")]
    Message(Message),
    #[serde(rename = "delProperty")]
    DelProperty(DelProperty),
    #[serde(rename = "enableBLOB")]
    EnableBlob(EnableBlob),

    // Commands from Connection to Device
    #[serde(rename = "getProperties")]
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
            timestamp: Some(chrono::offset::Utc::now().into()),
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
            timestamp: Some(chrono::offset::Utc::now().into()),
            switches: self,
        })
    }
}

impl ToCommand<Vec<(&str, f64)>> for Vec<(&str, f64)> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewNumberVector(NewNumberVector {
            device: device_name,
            name: param_name,
            timestamp: Some(Timestamp(chrono::offset::Utc::now())),
            numbers: self
                .iter()
                .map(|x| OneNumber {
                    name: String::from(x.0),
                    value: x.1.into(),
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
            timestamp: Some(Timestamp(chrono::offset::Utc::now())),
            numbers: self,
        })
    }
}

impl ToCommand<Vec<(&str, &str)>> for Vec<(&str, &str)> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewTextVector(NewTextVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now().into()),
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
            timestamp: Some(chrono::offset::Utc::now().into()),
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

#[derive(Debug, Deserialize)]
pub struct DefTextVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@label")]
    pub label: Option<String>,
    #[serde(rename = "@group")]
    pub group: Option<String>,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@perm")]
    pub perm: PropertyPerm,
    #[serde(rename = "@timeout")]
    pub timeout: Option<u32>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "defText")]
    pub texts: Vec<DefText>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct DefText {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@label")]
    pub label: Option<String>,
    #[serde(rename = "$text", default = "String::new")]
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct SetTextVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@timeout")]
    pub timeout: Option<u32>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "oneText")]
    pub texts: Vec<OneText>,
}

#[derive(Debug, Deserialize)]
pub struct NewTextVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,

    #[serde(rename = "oneText")]
    pub texts: Vec<OneText>,
}

#[derive(Debug, PartialEq, Clone, Deserialize)]
pub struct OneText {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "$text", default = "String::new")]
    pub value: String,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Sexagesimal {
    pub hour: f64,
    pub minute: Option<f64>,
    pub second: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct DefNumberVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@label")]
    pub label: Option<String>,
    #[serde(rename = "@group")]
    pub group: Option<String>,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@perm")]
    pub perm: PropertyPerm,
    #[serde(rename = "@timeout")]
    pub timeout: Option<u32>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "defNumber")]
    pub numbers: Vec<DefNumber>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct DefNumber {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@label")]
    pub label: Option<String>,
    #[serde(rename = "@format")]
    pub format: String,
    #[serde(rename = "@min")]
    pub min: f64,
    #[serde(rename = "@max")]
    pub max: f64,
    #[serde(rename = "@step")]
    pub step: f64,
    #[serde(rename = "$value")]
    pub value: Sexagesimal,
}

#[derive(Debug, Deserialize)]
pub struct SetNumberVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@timeout")]
    pub timeout: Option<u32>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "oneNumber")]
    pub numbers: Vec<SetOneNumber>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct SetOneNumber {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@min")]
    pub min: Option<f64>,
    #[serde(rename = "@max")]
    pub max: Option<f64>,
    #[serde(rename = "@step")]
    pub step: Option<f64>,
    #[serde(rename = "$value")]
    pub value: Sexagesimal,
}

#[derive(Debug, Deserialize)]
pub struct NewNumberVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,

    #[serde(rename = "oneNumber")]
    pub numbers: Vec<OneNumber>,
}

#[derive(Debug, PartialEq, Clone, Deserialize)]
pub struct OneNumber {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "$value")]
    pub value: Sexagesimal,
}

#[derive(Debug, Deserialize)]
pub struct DefSwitchVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@label")]
    pub label: Option<String>,
    #[serde(rename = "@group")]
    pub group: Option<String>,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@perm")]
    pub perm: PropertyPerm,
    #[serde(rename = "@rule")]
    pub rule: SwitchRule,
    #[serde(rename = "@timeout")]
    pub timeout: Option<u32>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "defSwitch")]
    pub switches: Vec<DefSwitch>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct DefSwitch {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@label")]
    pub label: Option<String>,
    #[serde(rename = "$text")]
    pub value: SwitchState,
}

#[derive(Debug, Deserialize)]
pub struct SetSwitchVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@timeout")]
    pub timeout: Option<u32>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "oneSwitch")]
    pub switches: Vec<OneSwitch>,
}
#[derive(Debug, Deserialize)]
pub struct NewSwitchVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,

    #[serde(rename = "oneSwitch")]
    pub switches: Vec<OneSwitch>,
}

#[derive(Debug, PartialEq, Clone, Deserialize)]
pub struct OneSwitch {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "$text")]
    pub value: SwitchState,
}

#[derive(Debug, Deserialize)]
pub struct DefLightVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@label")]
    pub label: Option<String>,
    #[serde(rename = "@group")]
    pub group: Option<String>,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "defLight")]
    pub lights: Vec<DefLight>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct DefLight {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@label")]
    label: Option<String>,
    #[serde(rename = "$text")]
    value: PropertyState,
}

#[derive(Debug, Deserialize)]
pub struct SetLightVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "oneLight")]
    pub lights: Vec<OneLight>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct OneLight {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "$text")]
    value: PropertyState,
}

#[derive(Debug, Deserialize)]
pub struct DefBlobVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@label")]
    pub label: Option<String>,
    #[serde(rename = "@group")]
    pub group: Option<String>,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@perm")]
    pub perm: PropertyPerm,
    #[serde(rename = "@timeout")]
    pub timeout: Option<u32>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "defBLOB")]
    pub blobs: Vec<DefBlob>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct DefBlob {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@label")]
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetBlobVector {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@state")]
    pub state: PropertyState,
    #[serde(rename = "@timeout")]
    pub timeout: Option<u32>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,

    #[serde(rename = "oneBLOB")]
    pub blobs: Vec<OneBlob>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Blob(pub Vec<u8>);

#[derive(Debug, PartialEq, Deserialize)]
pub struct OneBlob {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@size")]
    pub size: u64,
    #[serde(rename = "@enclen")]
    pub enclen: Option<u64>,
    #[serde(rename = "@format")]
    pub format: String,
    #[serde(rename = "$text")]
    pub value: Blob,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct EnableBlob {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: Option<String>,

    #[serde(rename = "$text")]
    pub enabled: BlobEnable,
}

#[derive(Debug, PartialEq, Clone, Deserialize)]
pub struct Message {
    #[serde(rename = "@device")]
    pub device: Option<String>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct DelProperty {
    #[serde(rename = "@device")]
    pub device: String,
    #[serde(rename = "@name")]
    pub name: Option<String>,
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "@message")]
    pub message: Option<String>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct GetProperties {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@device")]
    pub device: Option<String>,
    #[serde(rename = "@name")]
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
    XmlDeError(quick_xml::DeError),
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

impl From<quick_xml::DeError> for DeError {
    fn from(err: quick_xml::DeError) -> Self {
        DeError::XmlDeError(err)
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

pub struct CommandIter<'a, T: XmlRead<'a>> {
    xml_reader: quick_xml::de::Deserializer<'a, T>,
}

impl<'a, T: XmlRead<'a>> Iterator for CommandIter<'a, T> {
    type Item = Result<Command, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match Command::deserialize(&mut self.xml_reader) {
            Ok(command) => Some(Ok(command)),
            Err(quick_xml::DeError::UnexpectedEof) => None,
            Err(e) => Some(Err(e.into())),
        }
    }
}

impl<'a, T: BufRead> CommandIter<'a, IoReader<T>> {
    pub fn new(xml_reader: T) -> CommandIter<'a, IoReader<T>> {
        CommandIter {
            xml_reader: quick_xml::de::Deserializer::from_reader(xml_reader),
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    #[tokio::test]
    pub async fn play() {
        let xml = r#"
        <message message="msg 1"/>
        <message message="msg 1"/>
    "#
        .as_bytes();

        let c = Cursor::new(xml);
        let mut de = quick_xml::de::Deserializer::from_reader(c);

        let m = Message::deserialize(&mut de);
        dbg!(&m);
        let m = Message::deserialize(&mut de);
        dbg!(&m);
    }

    #[test]
    pub fn test_command() {
        let xml = r#"
    <message message="msg 1"/>
    <message message="msg 1"/>
"#;
        let mut des = quick_xml::de::Deserializer::from_str(xml);
        let command: Command = Command::deserialize(&mut des).unwrap();

        if let Command::Message(m) = command {
            assert_eq!(
                m,
                Message {
                    device: None,
                    timestamp: None,
                    message: Some("msg 1".into())
                }
            )
        }
    }
}
