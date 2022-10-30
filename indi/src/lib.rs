use quick_xml::events;
use quick_xml::events::attributes::AttrError;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::BytesText;
use quick_xml::events::Event;
use quick_xml::Result as XmlResult;
use quick_xml::{Reader, Writer};

use derivative::Derivative;

use std::borrow::Cow;
use std::net::{Shutdown, TcpStream};
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
    pub name: String,
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
    pub name: String,
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
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,

    pub values: HashMap<String, Blob>,
}

#[derive(Debug, PartialEq)]
pub enum Parameter {
    TextVector(TextVector),
    NumberVector(NumberVector),
    SwitchVector(SwitchVector),
    BlobVector(BlobVector),
}

#[derive(Debug, PartialEq)]
pub enum UpdateError {
    ParameterMissing(String),
    ParameterTypeMismatch(String),
}

#[derive(Debug)]
pub struct Device {
    parameters: HashMap<String, Parameter>,
}

impl Device {
    pub fn new() -> Device {
        Device {
            parameters: HashMap::new(),
        }
    }

    pub fn update(
        &mut self,
        command: serialization::Command,
    ) -> Result<Option<&Parameter>, UpdateError> {
        match command {
            Command::DefSwitchVector(def_command) => self.new_param(def_command),
            Command::SetSwitchVector(_) => Ok(None),
            Command::NewSwitchVector(new_command) => self.update_param(new_command),
            Command::DefNumberVector(def_command) => self.new_param(def_command),
            Command::SetNumberVector(_) => Ok(None),
            Command::NewNumberVector(new_command) => self.update_param(new_command),
            Command::DefTextVector(def_command) => self.new_param(def_command),
            Command::SetTextVector(_) => Ok(None),
            Command::NewTextVector(new_command) => self.update_param(new_command),
            Command::DelProperty(del_command) => {
                match del_command.name {
                    Some(name) => {
                        self.parameters.remove(&name);
                        ()
                    },
                    None => {
                        self.parameters.drain();
                        ()
                    }

                }
                Ok(None)
            }
            unhandled => panic!("Unhandled: {:?}", unhandled),
        }
    }

    pub fn get_parameters(&self) -> &HashMap<String, Parameter> {
        return &self.parameters;
    }

    fn new_param<T: CommandtoParam>(&mut self, def: T) -> Result<Option<&Parameter>, UpdateError> {
        let name = def.get_name().clone();
        let param = def.to_param();
        self.parameters.insert(name.clone(), param);
        Ok(self.parameters.get(&name))
    }

    fn update_param<T: CommandToUpdate>(
        &mut self,
        new_command: T,
    ) -> Result<Option<&Parameter>, UpdateError> {
        match self.parameters.get_mut(&new_command.get_name().clone()) {
            Some(param) => {
                new_command.update(param)?;
                Ok(Some(param))
            }
            None => Err(UpdateError::ParameterMissing(
                new_command.get_name().clone(),
            )),
        }
    }
}

trait CommandtoParam {
    fn get_name(&self) -> &String;
    fn to_param(self) -> Parameter;
}

trait CommandToUpdate {
    fn get_name(&self) -> &String;
    fn update(self, switch_vector: &mut Parameter) -> Result<String, UpdateError>;
}

#[derive(Debug)]
pub struct Client {
    devices: HashMap<String, Device>,
}

impl Client {
    pub fn new() -> Client {
        Client {
            devices: HashMap::new(),
        }
    }

    pub fn update(&mut self,
        command: serialization::Command,
    ) -> Result<Option<&Parameter>, UpdateError> {
        let name = command.device_name();
        match name {
            Some(name) => {
                let device = self.devices.entry(name.clone()).or_insert(Device::new());
                device.update(command)
            },
            None => Ok(None)
        }
    }

    pub fn get_devices(&self) -> &HashMap<String, Device> {
        return &self.devices;
    }

    pub fn clear(&mut self) {
        self.devices.clear();
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Connection {
    connection: TcpStream,
    #[derivative(Debug="ignore")]
    xml_writer: Writer<BufWriter<TcpStream>>,
}

impl Connection {
    pub fn new(addr: &str) -> std::io::Result<Connection> {
        let connection = TcpStream::connect(addr)?;
        let xml_writer = Writer::new_with_indent(BufWriter::new(connection.try_clone()?), b' ', 2);

        Ok(Connection {
            connection,
            xml_writer,
        })
    }

    pub fn disconnect(&self) -> Result<(), std::io::Error> {
        self.connection.shutdown(Shutdown::Both)
    }

    pub fn command_iter(
        &self,
    ) -> Result<serialization::CommandIter<BufReader<TcpStream>>, std::io::Error> {
        let mut xml_reader = Reader::from_reader(BufReader::new(self.connection.try_clone()?));
        xml_reader.trim_text(true);
        xml_reader.expand_empty_elements(true);
        Ok(serialization::CommandIter::new(xml_reader))
    }

    pub fn send<T: XmlSerialization>(&mut self, command: &T) -> Result<(), DeError> {
        command.send(&mut self.xml_writer)?;
        self.xml_writer.inner().flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod device_tests {
    use super::*;

    #[test]
    fn test_update_switch() {
        let mut device = Device::new();
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

        let def_switch = DefSwitchVector {
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
        assert_eq!(device.get_parameters().len(), 0);
        device
            .update(serialization::Command::DefSwitchVector(def_switch))
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        if let Parameter::SwitchVector(stored) = device.get_parameters().get("Exposure").unwrap() {
            assert_eq!(
                stored,
                &SwitchVector {
                    name: String::from_str("Exposure").unwrap(),
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
        } else {
            panic!("Unexpected");
        }

        let timestamp = DateTime::from_str("2022-10-13T08:41:56.301Z").unwrap();
        let new_switch = NewSwitchVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            timestamp: Some(timestamp),
            switches: vec![OneSwitch {
                name: String::from_str("seconds").unwrap(),
                value: SwitchState::Off,
            }],
        };
        assert_eq!(device.get_parameters().len(), 1);
        device
            .update(serialization::Command::NewSwitchVector(new_switch))
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        if let Parameter::SwitchVector(stored) = device.get_parameters().get("Exposure").unwrap() {
            assert_eq!(
                stored,
                &SwitchVector {
                    name: String::from_str("Exposure").unwrap(),
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
                            value: SwitchState::Off
                        }
                    )])
                }
            );
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_update_number() {
        let mut device = Device::new();
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

        let def_number = DefNumberVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            label: Some(String::from_str("thingo").unwrap()),
            group: Some(String::from_str("group").unwrap()),
            state: PropertyState::Ok,
            perm: PropertyPerm::RW,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            numbers: vec![DefNumber {
                name: String::from_str("seconds").unwrap(),
                label: Some(String::from_str("asdf").unwrap()),
                format: String::from_str("%4.0f").unwrap(),
                min: 0.0,
                max: 100.0,
                step: 1.0,
                value: 13.3,
            }],
        };
        assert_eq!(device.get_parameters().len(), 0);
        device
            .update(serialization::Command::DefNumberVector(def_number))
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        if let Parameter::NumberVector(stored) = device.get_parameters().get("Exposure").unwrap() {
            assert_eq!(
                stored,
                &NumberVector {
                    name: String::from_str("Exposure").unwrap(),
                    group: Some(String::from_str("group").unwrap()),
                    label: Some(String::from_str("thingo").unwrap()),
                    state: PropertyState::Ok,
                    perm: PropertyPerm::RW,
                    timeout: Some(60),
                    timestamp: Some(timestamp),
                    values: HashMap::from([(
                        String::from_str("seconds").unwrap(),
                        Number {
                            label: Some(String::from_str("asdf").unwrap()),
                            format: String::from_str("%4.0f").unwrap(),
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            value: 13.3,
                        }
                    )])
                }
            );
        } else {
            panic!("Unexpected");
        }

        let timestamp = DateTime::from_str("2022-10-13T08:41:56.301Z").unwrap();
        let new_number = NewNumberVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            timestamp: Some(timestamp),
            numbers: vec![OneNumber {
                name: String::from_str("seconds").unwrap(),
                min: None,
                max: None,
                step: None,
                value: 5.0,
            }],
        };
        assert_eq!(device.get_parameters().len(), 1);
        device
            .update(serialization::Command::NewNumberVector(new_number))
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        if let Parameter::NumberVector(stored) = device.get_parameters().get("Exposure").unwrap() {
            assert_eq!(
                stored,
                &NumberVector {
                    name: String::from_str("Exposure").unwrap(),
                    group: Some(String::from_str("group").unwrap()),
                    label: Some(String::from_str("thingo").unwrap()),
                    state: PropertyState::Ok,
                    perm: PropertyPerm::RW,
                    timeout: Some(60),
                    timestamp: Some(timestamp),
                    values: HashMap::from([(
                        String::from_str("seconds").unwrap(),
                        Number {
                            label: Some(String::from_str("asdf").unwrap()),
                            format: String::from_str("%4.0f").unwrap(),
                            min: 0.0,
                            max: 100.0,
                            step: 1.0,
                            value: 5.0
                        }
                    )])
                }
            );
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_update_text() {
        let mut device = Device::new();
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

        let def_text = DefTextVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            label: Some(String::from_str("thingo").unwrap()),
            group: Some(String::from_str("group").unwrap()),
            state: PropertyState::Ok,
            perm: PropertyPerm::RW,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            texts: vec![DefText {
                name: String::from_str("seconds").unwrap(),
                label: Some(String::from_str("asdf").unwrap()),
                value: String::from_str("something").unwrap(),
            }],
        };
        assert_eq!(device.get_parameters().len(), 0);
        device
            .update(serialization::Command::DefTextVector(def_text))
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        if let Parameter::TextVector(stored) = device.get_parameters().get("Exposure").unwrap() {
            assert_eq!(
                stored,
                &TextVector {
                    name: String::from_str("Exposure").unwrap(),
                    group: Some(String::from_str("group").unwrap()),
                    label: Some(String::from_str("thingo").unwrap()),
                    state: PropertyState::Ok,
                    perm: PropertyPerm::RW,
                    timeout: Some(60),
                    timestamp: Some(timestamp),
                    values: HashMap::from([(
                        String::from_str("seconds").unwrap(),
                        Text {
                            label: Some(String::from_str("asdf").unwrap()),
                            value: String::from_str("something").unwrap(),
                        }
                    )])
                }
            );
        } else {
            panic!("Unexpected");
        }

        let timestamp = DateTime::from_str("2022-10-13T08:41:56.301Z").unwrap();
        let new_number = NewTextVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            timestamp: Some(timestamp),
            texts: vec![OneText {
                name: String::from_str("seconds").unwrap(),
                value: String::from_str("something else").unwrap(),
            }],
        };
        assert_eq!(device.get_parameters().len(), 1);
        device
            .update(serialization::Command::NewTextVector(new_number))
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        if let Parameter::TextVector(stored) = device.get_parameters().get("Exposure").unwrap() {
            assert_eq!(
                stored,
                &TextVector {
                    name: String::from_str("Exposure").unwrap(),
                    group: Some(String::from_str("group").unwrap()),
                    label: Some(String::from_str("thingo").unwrap()),
                    state: PropertyState::Ok,
                    perm: PropertyPerm::RW,
                    timeout: Some(60),
                    timestamp: Some(timestamp),
                    values: HashMap::from([(
                        String::from_str("seconds").unwrap(),
                        Text {
                            label: Some(String::from_str("asdf").unwrap()),
                            value: String::from_str("something else").unwrap(),
                        }
                    )])
                }
            );
        } else {
            panic!("Unexpected");
        }
    }
}
