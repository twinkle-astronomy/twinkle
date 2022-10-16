pub mod number_vector;
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

use quick_xml::Result as XmlResult;
use quick_xml::{Reader, Writer};

#[cfg(test)]
mod tests;

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
    pub name: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
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

#[derive(Debug, PartialEq)]
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

pub trait XmlSerialization {
    fn send<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>>;
}

#[derive(Debug)]
pub enum DeError {
    XmlError(quick_xml::Error),
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

impl<'a> TryFrom<Attribute<'a>> for SwitchRule {
    type Error = DeError;

    fn try_from(value: Attribute<'a>) -> Result<Self, Self::Error> {
        match value.unescaped_value()? {
            Cow::Borrowed(b"OneOfMany") => Ok(SwitchRule::OneOfMany),
            Cow::Borrowed(b"AtMostOne") => Ok(SwitchRule::AtMostOne),
            Cow::Borrowed(b"AnyOfMany") => Ok(SwitchRule::AnyOfMany),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}

impl<'a> TryFrom<Attribute<'a>> for PropertyState {
    type Error = DeError;

    fn try_from(value: Attribute<'a>) -> Result<Self, Self::Error> {
        match value.unescaped_value()? {
            Cow::Borrowed(b"Idle") => Ok(PropertyState::Idle),
            Cow::Borrowed(b"Ok") => Ok(PropertyState::Ok),
            Cow::Borrowed(b"Busy") => Ok(PropertyState::Busy),
            Cow::Borrowed(b"Alert") => Ok(PropertyState::Alert),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}

impl<'a> TryFrom<BytesText<'a>> for PropertyState {
    type Error = DeError;

    fn try_from(value: BytesText<'a>) -> Result<Self, Self::Error> {
        match value.unescaped()? {
            Cow::Borrowed(b"Idle") => Ok(PropertyState::Idle),
            Cow::Borrowed(b"Ok") => Ok(PropertyState::Ok),
            Cow::Borrowed(b"Busy") => Ok(PropertyState::Busy),
            Cow::Borrowed(b"Alert") => Ok(PropertyState::Alert),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}

impl<'a> TryFrom<BytesText<'a>> for SwitchState {
    type Error = DeError;

    fn try_from(value: BytesText<'a>) -> Result<Self, Self::Error> {
        match value.unescaped()? {
            Cow::Borrowed(b"On") => Ok(SwitchState::On),
            Cow::Borrowed(b"Off") => Ok(SwitchState::Off),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}
impl<'a> TryFrom<Attribute<'a>> for PropertyPerm {
    type Error = DeError;

    fn try_from(value: Attribute<'a>) -> Result<Self, Self::Error> {
        match value.unescaped_value()? {
            Cow::Borrowed(b"ro") => Ok(PropertyPerm::RO),
            Cow::Borrowed(b"wo") => Ok(PropertyPerm::WO),
            Cow::Borrowed(b"rw") => Ok(PropertyPerm::RW),
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
        let event = self.xml_reader.read_event(&mut self.buf)?;
        match event {
            Event::Start(e) => {
                let result = match e.name() {
                    b"defTextVector" => {
                        let mut text_vector = DefTextIter::text_vector(&self.xml_reader, &e)?;

                        for text in DefTextIter::new(self) {
                            let text = text?;
                            text_vector.texts.push(text);
                        }

                        Ok(Some(Command::DefTextVector(text_vector)))
                    }
                    b"setTextVector" => {
                        let mut text_vector = SetTextIter::text_vector(&self.xml_reader, &e)?;

                        for text in SetTextIter::new(self) {
                            let text = text?;
                            text_vector.texts.push(text);
                        }

                        Ok(Some(Command::SetTextVector(text_vector)))
                    }
                    b"newTextVector" => {
                        let mut text_vector = NewTextIter::text_vector(&self.xml_reader, &e)?;

                        for text in NewTextIter::new(self) {
                            let text = text?;
                            text_vector.texts.push(text);
                        }

                        Ok(Some(Command::NewTextVector(text_vector)))
                    }
                    b"defNumberVector" => {
                        let mut number_vector = DefNumberIter::number_vector(&self.xml_reader, &e)?;

                        for number in DefNumberIter::new(self) {
                            let number = number?;
                            number_vector.numbers.push(number);
                        }

                        Ok(Some(Command::DefNumberVector(number_vector)))
                    }
                    b"setNumberVector" => {
                        let mut number_vector = SetNumberIter::number_vector(&self.xml_reader, &e)?;

                        for number in SetNumberIter::new(self) {
                            let number = number?;
                            number_vector.numbers.push(number);
                        }

                        Ok(Some(Command::SetNumberVector(number_vector)))
                    }
                    b"newNumberVector" => {
                        let mut number_vector = NewNumberIter::number_vector(&self.xml_reader, &e)?;

                        for number in NewNumberIter::new(self) {
                            let number = number?;
                            number_vector.numbers.push(number);
                        }

                        Ok(Some(Command::NewNumberVector(number_vector)))
                    }
                    b"defSwitchVector" => {
                        let mut switch_vector = DefSwitchIter::switch_vector(&self.xml_reader, &e)?;

                        for switch in DefSwitchIter::new(self) {
                            let switch = switch?;
                            switch_vector.switches.push(switch);
                        }

                        Ok(Some(Command::DefSwitchVector(switch_vector)))
                    }
                    b"setSwitchVector" => {
                        let mut switch_vector = SetSwitchIter::switch_vector(&self.xml_reader, &e)?;

                        for switch in SetSwitchIter::new(self) {
                            let switch = switch?;
                            switch_vector.switches.push(switch);
                        }

                        Ok(Some(Command::SetSwitchVector(switch_vector)))
                    }
                    b"newSwitchVector" => {
                        let mut switch_vector = NewSwitchIter::switch_vector(&self.xml_reader, &e)?;

                        for switch in NewSwitchIter::new(self) {
                            let switch = switch?;
                            switch_vector.switches.push(switch);
                        }

                        Ok(Some(Command::NewSwitchVector(switch_vector)))
                    }
                    b"defLightVector" => {
                        let mut light_vector = DefLightIter::light_vector(&self.xml_reader, &e)?;

                        for light in DefLightIter::new(self) {
                            let light = light?;
                            light_vector.lights.push(light);
                        }

                        Ok(Some(Command::DefLightVector(light_vector)))
                    }
                    b"setLightVector" => {
                        let mut light_vector = SetLightIter::light_vector(&self.xml_reader, &e)?;

                        for light in SetLightIter::new(self) {
                            let light = light?;
                            light_vector.lights.push(light);
                        }

                        Ok(Some(Command::SetLightVector(light_vector)))
                    }
                    b"defBLOBVector" => {
                        let mut blob_vector = DefBlobIter::blob_vector(&self.xml_reader, &e)?;

                        for blob in DefBlobIter::new(self) {
                            let blob = blob?;
                            blob_vector.blobs.push(blob);
                        }

                        Ok(Some(Command::DefBlobVector(blob_vector)))
                    }
                    b"setBLOBVector" => {
                        let mut blob_vector = SetBlobIter::blob_vector(&self.xml_reader, &e)?;

                        for blob in SetBlobIter::new(self) {
                            let blob = blob?;
                            blob_vector.blobs.push(blob);
                        }

                        Ok(Some(Command::SetBlobVector(blob_vector)))
                    }
                    b"message" => {
                        let message = MessageIter::message(&self.xml_reader, &e)?;
                        for _ in MessageIter::new(self) {}

                        Ok(Some(Command::Message(message)))
                    }
                    b"delProperty" => {
                        let message = DelPropertyIter::del_property(&self.xml_reader, &e)?;
                        for _ in DelPropertyIter::new(self) {}

                        Ok(Some(Command::DelProperty(message)))
                    }

                    b"getProperties" => {
                        let get_properties =
                            GetPropertiesIter::get_properties(&self.xml_reader, &e)?;
                        for _ in GetPropertiesIter::new(self) {}

                        Ok(Some(Command::GetProperties(get_properties)))
                    }
                    tag => Err(DeError::UnexpectedTag(str::from_utf8(tag)?.to_string())),
                };
                result
            }
            Event::End(tag) => {
                println!("Unexpected end: {}", tag.escape_ascii().to_string());
                Err(DeError::UnexpectedEvent(format!("{:?}", tag)))
            }
            Event::Eof => Ok(None),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}
