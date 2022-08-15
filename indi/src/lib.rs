use quick_xml::events;
use quick_xml::events::attributes::AttrError;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};

use std::collections::HashMap;
use std::net::TcpStream;

use std::io::{BufRead, BufReader, BufWriter, Write};

use std::num;
use std::str;

static INDI_PROTOCOL_VERSION: &str = "1.7";

pub struct Client {
    connection: TcpStream,
    xml_writer: Writer<BufWriter<TcpStream>>,
    devices: HashMap<String, Device>,
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
    pub timestamp: String,

    pub numbers: HashMap<String, Number>,
}

#[derive(Debug, PartialEq)]
pub struct Number {
    name: String,
    label: Option<String>,
    format: String,
    min: i32,
    max: i32,
    step: i32,
    value: f64,
}

#[derive(Debug)]
pub enum DeError {
    XmlError(quick_xml::Error),
    Decode(str::Utf8Error),
    ParseIntError(num::ParseIntError),
    ParseFloatError(num::ParseFloatError),
    MissingAttr(String),
    BadAttr(AttrError),
    UnexpectedAttr(String),
    UnexpectedEvent(),
}
impl From<quick_xml::Error> for DeError {
    fn from(err: quick_xml::Error) -> Self {
        DeError::XmlError(err)
    }
}
impl From<str::Utf8Error> for DeError {
    fn from(err: str::Utf8Error) -> Self {
        DeError::Decode(err)
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
impl From<AttrError> for DeError {
    fn from(err: AttrError) -> Self {
        DeError::BadAttr(err)
    }
}
// impl From<

impl Client {
    pub fn new(addr: &str) -> std::io::Result<Client> {
        let connection = TcpStream::connect(addr)?;
        let xml_writer = Writer::new_with_indent(BufWriter::new(connection.try_clone()?), b' ', 2);
        // let xml_reader = Reader::from_reader(BufReader::new(connection.try_clone()?));
        let devices = HashMap::new();
        Ok(Client {
            connection,
            xml_writer,
            devices,
        })
    }

    pub fn listen_for_updates(&mut self) {
        let xml_reader = Reader::from_reader(BufReader::new(self.connection.try_clone().unwrap()));
        if let Some(param) = listen_for_updates(xml_reader).unwrap() {
            let (device_name, param_name) = match param {
                Parameter::Number(ref param) => (param.device.clone(), param.name.clone()),
            };
            if let Some(device) = self.devices.get_mut(&device_name) {
                device.parameters.insert(param_name, param);
            } else {
                self.devices.insert(device_name.clone(), Device {
                    name: device_name,
                    parameters: HashMap::from([(param_name, param)]),
                });
            }
        }
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

fn listen_for_updates<T: BufRead>(mut xml_reader: Reader<T>) -> Result<Option<Parameter>, DeError> {
    let mut buf = Vec::new();
    loop {
        println!("Loop!");
        match xml_reader.read_event(&mut buf)? {
            Event::Start(e) => {
                match e.name() {
                    b"defNumberVector" => {
                        return Ok(Some(Parameter::Number(NumberVector::parse(
                            &mut xml_reader,
                            e,
                        )?)));
                    }
                    _ => panic!("foo"),
                }
            }
            Event::Eof => break Ok(None), // exits the loop when reaching end of file
            _ => (),
        }
    }
}

impl NumberVector {
    fn parse<T: BufRead>(
        mut xml_reader: &mut Reader<T>,
        start_event: events::BytesStart,
    ) -> Result<NumberVector, DeError> {
        let mut device: Option<String> = None;
        let mut name: Option<String> = None;
        let mut label: Option<String> = None;
        let mut group: Option<String> = None;
        let mut state: Option<String> = None;
        let mut perm: Option<String> = None;
        let mut timeout: Option<u32> = None;
        let mut timestamp: Option<String> = None;

        let mut buf = Vec::new();
        let mut numbers: HashMap<String, Number> = HashMap::new();

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.unescape_and_decode_value(&xml_reader)?;
            match str::from_utf8(attr.key)? {
                "device" => device = Some(attr_value),
                "name" => name = Some(attr_value),
                "label" => label = Some(attr_value),
                "group" => group = Some(attr_value),
                "state" => state = Some(attr_value),
                "perm" => perm = Some(attr_value),
                "timeout" => timeout = Some(attr_value.parse::<u32>()?),
                "timestamp" => timestamp = Some(attr_value),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        key
                    )))
                }
            }
        }

        loop {
            match xml_reader.read_event(&mut buf) {
                Ok(Event::Start(start_event)) => {
                    let number = Number::parse(&mut xml_reader, start_event)?;
                    numbers.insert(number.name.clone(), number);
                }
                Ok(Event::End(_)) => break,
                _ => return Err(DeError::UnexpectedEvent()),
            }
        }

        return Ok(NumberVector {
            device: device.ok_or(DeError::MissingAttr("device".to_string()))?,
            name: name.ok_or(DeError::MissingAttr("name".to_string()))?,
            label: label.ok_or(DeError::MissingAttr("label".to_string()))?,
            group: group.ok_or(DeError::MissingAttr("group".to_string()))?,
            state: state.ok_or(DeError::MissingAttr("state".to_string()))?,
            perm: perm.ok_or(DeError::MissingAttr("perm".to_string()))?,
            timeout: timeout.ok_or(DeError::MissingAttr("timeout".to_string()))?,
            timestamp: timestamp.ok_or(DeError::MissingAttr("timeout".to_string()))?,
            numbers: numbers,
        });
    }
}

impl Number {
    fn parse<T: BufRead>(
        xml_reader: &mut Reader<T>,
        start_event: events::BytesStart,
    ) -> Result<Number, DeError> {
        let mut name: Option<String> = None;
        let mut label: Option<String> = None;
        let mut format: Option<String> = None;
        let mut min: Option<i32> = None;
        let mut max: Option<i32> = None;
        let mut step: Option<i32> = None;
        let mut value: Option<f64> = None;

        let mut buf = Vec::new();

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.unescape_and_decode_value(&xml_reader)?;
            match str::from_utf8(attr.key)? {
                "name" => name = Some(attr_value),
                "label" => label = Some(attr_value),
                "format" => format = Some(attr_value),
                "min" => min = Some(attr_value.parse::<i32>()?),
                "max" => max = Some(attr_value.parse::<i32>()?),
                "step" => step = Some(attr_value.parse::<i32>()?),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        key
                    )))
                }
            }
        }

        loop {
            match xml_reader.read_event(&mut buf) {
                Ok(Event::Text(e)) => {
                    value = Some(e.unescape_and_decode(&xml_reader)?.parse::<f64>()?)
                }
                Ok(Event::End(_)) => break,
                _ => return Err(DeError::UnexpectedEvent()),
            }
        }
        return Ok(Number {
            name: name.ok_or(DeError::MissingAttr("name".to_string()))?,
            label: label,
            format: format.ok_or(DeError::MissingAttr("format".to_string()))?,
            min: min.ok_or(DeError::MissingAttr("min".to_string()))?,
            max: max.ok_or(DeError::MissingAttr("max".to_string()))?,
            step: step.ok_or(DeError::MissingAttr("step".to_string()))?,
            value: value.ok_or(DeError::MissingAttr("value".to_string()))?,
        });
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

            match listen_for_updates(reader).unwrap().unwrap() {
                Parameter::Number(param) => {
                    assert_eq!(param.device, "CCD Simulator");    
                },
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
        assert_eq!(result.timestamp, "2022-08-12T05:52:27".to_string());
    }
}
