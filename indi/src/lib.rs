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

use std::collections::HashMap;

pub static INDI_PROTOCOL_VERSION: &str = "1.7";

pub mod serialization;
pub use serialization::*;

#[derive(Debug, PartialEq)]
pub struct Switch {
    pub label: Option<String>,
    pub value: SwitchState,
}

#[derive(Debug, PartialEq)]
pub struct SwitchVector {
    pub group: Option<String>,
    pub label: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub rule: SwitchRule,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Switch>,
}

#[derive(Debug, PartialEq)]
pub struct Number {
    pub label: Option<String>,
    pub format: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: f64,
}
#[derive(Debug, PartialEq)]
pub struct NumberVector {
    pub group: Option<String>,
    pub label: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Number>,
}

#[derive(Debug, PartialEq)]
pub struct Text {
    pub label: Option<String>,
    pub value: String,
}

#[derive(Debug, PartialEq)]
pub struct TextVector {
    pub group: Option<String>,
    pub label: Option<String>,

    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Text>,
}

#[derive(Debug, PartialEq)]
pub struct Blob {
    pub label: Option<String>,
    pub format: String,
    pub value: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq)]
pub struct BlobVector {
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,

    pub values: HashMap<String, Blob>,
}

#[derive(Debug, PartialEq)]
pub enum Properties {
    TextVector(TextVector),
    NumberVector(NumberVector),
    SwitchVector(SwitchVector),
    BlobVector(BlobVector),
}

pub struct Device {
    properties: HashMap<String, Properties>,
}

impl Device {
    pub fn new() -> Device {
        Device {
            properties: HashMap::new(),
        }
    }

    pub fn update(&mut self, command: serialization::Command) {
        if let Command::DefSwitchVector(def_param) = command {
            let (name, param) = def_param.switch_vector();
            self.properties
                .insert(name, Properties::SwitchVector(param));
        }
    }

    pub fn get_properties(&self) -> &HashMap<String, Properties> {
        return &self.properties;
    }
}

#[cfg(test)]
mod device_tests {
    use super::*;

    #[test]
    fn test_update_switch() {
        let mut device = Device::new();
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

        let new_switch = DefSwitchVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            label: Some(String::from_str("thingo").unwrap()),
            group: Some(String::from_str("group").unwrap()),
            state: PropertyState::Ok,
            perm: PropertyPerm::RW,
            rule: SwitchRule::AtMostOne,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            switches: vec![DefSwitch {
                name: String::from_str("seconds").unwrap(),
                label: Some(String::from_str("asdf").unwrap()),
                value: SwitchState::On,
            }],
        };
        assert_eq!(device.get_properties().len(), 0);
        device.update(serialization::Command::DefSwitchVector(new_switch));
        assert_eq!(device.get_properties().len(), 1);

        if let Properties::SwitchVector(stored) = device.get_properties().get("Exposure").unwrap() {
            assert_eq!(
                stored,
                &SwitchVector {
                    group: Some(String::from_str("group").unwrap()),
                    label: Some(String::from_str("thingo").unwrap()),
                    state: PropertyState::Ok,
                    perm: PropertyPerm::RW,
                    rule: SwitchRule::AtMostOne,
                    timeout: Some(60),
                    timestamp: Some(timestamp),
                    values: HashMap::from([(
                        String::from_str("seconds").unwrap(),
                        Switch {
                            label: Some(String::from_str("asdf").unwrap()),
                            value: SwitchState::On
                        }
                    )])
                }
            );
        }
    }
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
