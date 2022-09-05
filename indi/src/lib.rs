use quick_xml::events;
use quick_xml::events::attributes::AttrError;
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
    devices: HashMap<String, Device>,
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
    Number(NumberVector),
}

#[derive(Debug)]
pub struct NumberVector {
    pub device: String,
    pub name: String,
    pub label: String,
    pub group: String,
    pub state: String,
    pub perm: String,
    pub timeout: u32,
    pub timestamp: DateTime<Utc>,

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_listen_for_updates() {
        let xml = r#"
    <defNumberVector device="CCD Simulator" name="SIMULATOR_SETTINGS" label="Settings" group="Simulator Config" state="Idle" perm="rw" timeout="60" timestamp="2022-08-12T05:52:27">
        <defNumber name="SIM_XRES" label="CCD X resolution" format="%4.0f" min="512" max="8192" step="512">
    1280
        </defNumber>
        <defNumber name="SIM_YRES" label="CCD Y resolution" format="%4.0f" min="512" max="8192" step="512">
    1024
        </defNumber>
        <defNumber name="SIM_XSIZE" label="CCD X Pixel Size" format="%4.2f" min="1" max="30" step="5">
    5.2000000000000001776
        </defNumber>
    </defNumberVector>
                    "#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        match listen_for_updates(&mut reader).unwrap().unwrap() {
            Parameter::Number(param) => {
                assert_eq!(param.device, "CCD Simulator");
            }
        }
    }

    #[test]
    fn test_parse_number() {
        let mut buf = Vec::new();
        let xml = r#"
    <defNumber name="SIM_XRES" label="CCD X resolution" format="%4.0f" min="512" max="8192" step="512">
1280
    </defNumber>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let result = match reader.read_event(&mut buf).unwrap() {
            Event::Start(start_event) => Number::parse(&mut reader, start_event).unwrap(),
            other => panic!("wrong element type to begin: {:?}", other),
        };
        // let result = Number::parse(reader).unwrap();
        assert_eq!(
            result,
            Number {
                name: "SIM_XRES".to_string(),
                label: Some("CCD X resolution".to_string()),
                format: "%4.0f".to_string(),
                min: 512,
                max: 8192,
                step: 512,
                value: 1280.0
            }
        );

        let xml = r#"
    <defNumber name="SIM_XSIZE" label="CCD X Pixel Size" format="%4.2f" min="1" max="30" step="5">
5.2000000000000001776
    </defNumber>
"#;
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let result = match reader.read_event(&mut buf).unwrap() {
            Event::Start(start_event) => Number::parse(&mut reader, start_event).unwrap(),
            other => panic!("wrong element type to begin: {:?}", other),
        };

        assert_eq!(
            result,
            Number {
                name: "SIM_XSIZE".to_string(),
                label: Some("CCD X Pixel Size".to_string()),
                format: "%4.2f".to_string(),
                min: 1,
                max: 30,
                step: 5,
                value: 5.2000000000000001776
            }
        );
    }

    #[test]
    fn test_parse_numbervector() {
        let mut buf = Vec::new();
        let xml = r#"
<defNumberVector device="CCD Simulator" name="SIMULATOR_SETTINGS" label="Settings" group="Simulator Config" state="Idle" perm="rw" timeout="60" timestamp="2022-08-12T05:52:27">
    <defNumber name="SIM_XRES" label="CCD X resolution" format="%4.0f" min="512" max="8192" step="512">
1280
    </defNumber>
    <defNumber name="SIM_YRES" label="CCD Y resolution" format="%4.0f" min="512" max="8192" step="512">
1024
    </defNumber>
    <defNumber name="SIM_XSIZE" label="CCD X Pixel Size" format="%4.2f" min="1" max="30" step="5">
5.2000000000000001776
    </defNumber>
</defNumberVector>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let result = match reader.read_event(&mut buf).unwrap() {
            Event::Start(start_event) => NumberVector::parse(&mut reader, start_event).unwrap(),
            _ => panic!("wrong element type"),
        };
        // let result = Number::parse(reader).unwrap();
        assert_eq!(result.name, "SIMULATOR_SETTINGS".to_string());
        assert_eq!(result.device, "CCD Simulator".to_string());
        assert_eq!(result.label, "Settings".to_string());
        assert_eq!(result.group, "Simulator Config".to_string());
        assert_eq!(result.state, "Idle".to_string());
        assert_eq!(result.perm, "rw".to_string());
        assert_eq!(result.timeout, 60);
        assert_eq!(
            result.timestamp,
            DateTime::<Utc>::from_str("2022-08-12T05:52:27Z").unwrap()
        );
    }
}
