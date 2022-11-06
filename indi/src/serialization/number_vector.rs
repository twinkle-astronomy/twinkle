use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;

use std::str;

// use encoding::all::ISO_8859_1;
// use encoding::{DecoderTrap, Encoding};

use super::super::*;
use super::*;

impl CommandtoParam for DefNumberVector {
    fn get_name(&self) -> &String {
        &self.name
    }
    fn to_param(self) -> Parameter {
        Parameter::NumberVector(NumberVector {
            name: self.name,
            group: self.group,
            label: self.label,
            state: self.state,
            perm: self.perm,
            timeout: self.timeout,
            timestamp: self.timestamp,
            values: self
                .numbers
                .into_iter()
                .map(|i| {
                    (
                        i.name,
                        Number {
                            label: i.label,
                            format: i.format,
                            min: i.min,
                            max: i.max,
                            step: i.step,
                            value: i.value,
                        },
                    )
                })
                .collect(),
        })
    }
}

impl CommandToUpdate for SetNumberVector {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn update(self, param: &mut Parameter) -> Result<String, UpdateError> {
        match param {
            Parameter::NumberVector(number_vector) => {
                number_vector.state = self.state;
                number_vector.timeout = self.timeout;
                number_vector.timestamp = self.timestamp;
                for number in self.numbers {
                    if let Some(existing) = number_vector.values.get_mut(&number.name) {
                        existing.min = number.min.unwrap_or(existing.min);
                        existing.max = number.max.unwrap_or(existing.max);
                        existing.step = number.step.unwrap_or(existing.step);
                        existing.value = number.value;
                    }
                }
                Ok(self.name)
            }
            _ => Err(UpdateError::ParameterTypeMismatch(self.name.clone())),
        }
    }
}

impl XmlSerialization for OneNumber {
    fn send<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let mut creator = xml_writer
            .create_element("oneNumber")
            .with_attribute(("name", &*self.name));

        if let Some(min) = &self.min {
            creator = creator.with_attribute(("min", min.to_string().as_str()));
        }
        if let Some(max) = &self.max {
            creator = creator.with_attribute(("max", max.to_string().as_str()));
        }
        if let Some(step) = &self.step {
            creator = creator.with_attribute(("step", step.to_string().as_str()));
        }
        creator.write_text_content(BytesText::new(self.value.to_string().as_str()))?;

        Ok(xml_writer)
    }
}

impl XmlSerialization for NewNumberVector {
    fn send<'a, T: std::io::Write>(
        &self,
        mut xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        {
            let mut creator = xml_writer
                .create_element("newNumberVector")
                .with_attribute(("device", &*self.device))
                .with_attribute(("name", &*self.name));

            if let Some(timestamp) = &self.timestamp {
                creator = creator.with_attribute((
                    "timestamp",
                    format!("{}", timestamp.format("%Y-%m-%dT%H:%M:%S%.3f")).as_str(),
                ));
            }
            xml_writer = creator.write_inner_content(|xml_writer| {
                for number in self.numbers.iter() {
                    number.send(xml_writer)?;
                }
                Ok(())
            })?;
        }

        Ok(xml_writer)
    }
}

fn parse_number(e: &BytesText) -> Result<f64, DeError> {
    let text = &e.unescape()?;
    let mut components = text.split([' ', ':']);

    let mut val: f64 = match components.next() {
        Some(comp) => comp.parse::<f64>()?,
        None => return Err(DeError::ParseSexagesimalError(text.to_string())),
    };

    let sign = val.signum();
    let mut div = 60.0;

    for comp in components {
        val += sign * comp.parse::<f64>()? / div;

        div = div * 60.0;
    }
    return Ok(val);
}

fn next_one_number<T: std::io::BufRead>(
    xml_reader: &mut Reader<T>,
    buf: &mut Vec<u8>,
) -> Result<Option<OneNumber>, DeError> {
    let event = xml_reader.read_event_into(buf)?;
    match event {
        Event::Start(e) => match e.name() {
            QName(b"oneNumber") => {
                let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));
                let mut min: Option<f64> = None;
                let mut max: Option<f64> = None;
                let mut step: Option<f64> = None;

                for attr in e.attributes() {
                    let attr = attr?;
                    let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();

                    match attr.key {
                        QName(b"name") => name = Ok(attr_value),
                        QName(b"min") => min = Some(attr_value.parse::<f64>()?),
                        QName(b"max") => max = Some(attr_value.parse::<f64>()?),
                        QName(b"step") => step = Some(attr_value.parse::<f64>()?),
                        key => {
                            return Err(DeError::UnexpectedAttr(format!(
                                "Unexpected attribute {}",
                                str::from_utf8(key.into_inner())?
                            )))
                        }
                    }
                }

                let value: Result<f64, DeError> = match xml_reader.read_event_into(buf) {
                    Ok(Event::Text(e)) => parse_number(&e),
                    e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                };

                let trailing_event = xml_reader.read_event_into(buf)?;
                match trailing_event {
                    Event::End(_) => (),
                    e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                }

                Ok(Some(OneNumber {
                    name: name?,
                    min: min,
                    max: max,
                    step: step,
                    value: value?,
                }))
            }
            tag => Err(DeError::UnexpectedTag(
                str::from_utf8(tag.into_inner())?.to_string(),
            )),
        },
        Event::End(_) => Ok(None),
        Event::Eof => Ok(None),
        e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
    }
}

pub struct DefNumberIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for DefNumberIter<'a, T> {
    type Item = Result<DefNumber, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_number() {
            Ok(Some(number)) => {
                return Some(Ok(number));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}
impl<'a, T: std::io::BufRead> DefNumberIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> DefNumberIter<T> {
        DefNumberIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn number_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<DefNumberVector, DeError> {
        let mut device: Option<String> = None;
        let mut name: Option<String> = None;
        let mut label: Option<String> = None;
        let mut group: Option<String> = None;
        let mut state: Option<PropertyState> = None;
        let mut perm: Option<PropertyPerm> = None;
        let mut timeout: Option<u32> = None;
        let mut timestamp: Option<DateTime<Utc>> = None;
        let mut message: Option<String> = None;

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Some(attr_value),
                QName(b"name") => name = Some(attr_value),
                QName(b"label") => label = Some(attr_value),
                QName(b"group") => group = Some(attr_value),
                QName(b"state") => state = Some(PropertyState::try_from(attr, xml_reader)?),
                QName(b"perm") => perm = Some(PropertyPerm::try_from(attr, xml_reader)?),
                QName(b"timeout") => timeout = Some(attr_value.parse::<u32>()?),
                QName(b"timestamp") => {
                    timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?)
                }
                QName(b"message") => message = Some(attr_value),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key.into_inner())?
                    )))
                }
            }
        }
        Ok(DefNumberVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            label: label,
            group: group,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            perm: perm.ok_or(DeError::MissingAttr(&"perm"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            numbers: Vec::new(),
        })
    }

    fn next_number(&mut self) -> Result<Option<DefNumber>, DeError> {
        let event = self.xml_reader.read_event_into(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                QName(b"defNumber") => {
                    let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));
                    let mut label: Option<String> = None;
                    let mut format: Result<String, DeError> = Err(DeError::MissingAttr(&"format"));
                    let mut min: Result<f64, DeError> = Err(DeError::MissingAttr(&"min"));
                    let mut max: Result<f64, DeError> = Err(DeError::MissingAttr(&"max"));
                    let mut step: Result<f64, DeError> = Err(DeError::MissingAttr(&"step"));

                    for attr in e.attributes() {
                        let attr = attr?;
                        let attr_value = attr
                            .decode_and_unescape_value(self.xml_reader)?
                            .into_owned();

                        match attr.key {
                            QName(b"name") => name = Ok(attr_value),
                            QName(b"label") => label = Some(attr_value),
                            QName(b"format") => format = Ok(attr_value),
                            QName(b"min") => min = Ok(attr_value.parse::<f64>()?),
                            QName(b"max") => max = Ok(attr_value.parse::<f64>()?),
                            QName(b"step") => step = Ok(attr_value.parse::<f64>()?),
                            key => {
                                return Err(DeError::UnexpectedAttr(format!(
                                    "Unexpected attribute {}",
                                    str::from_utf8(key.into_inner())?
                                )))
                            }
                        }
                    }

                    let value: Result<f64, DeError> =
                        match self.xml_reader.read_event_into(self.buf) {
                            Ok(Event::Text(e)) => parse_number(&e),
                            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                        };

                    let trailing_event = self.xml_reader.read_event_into(&mut self.buf)?;
                    match trailing_event {
                        Event::End(_) => (),
                        e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                    }

                    Ok(Some(DefNumber {
                        name: name?,
                        label: label,
                        format: format?,
                        min: min?,
                        max: max?,
                        step: step?,
                        value: value?,
                    }))
                }
                tag => Err(DeError::UnexpectedTag(
                    str::from_utf8(tag.into_inner())?.to_string(),
                )),
            },
            Event::End(_) => Ok(None),
            Event::Eof => Ok(None),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}

pub struct SetNumberIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for SetNumberIter<'a, T> {
    type Item = Result<OneNumber, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match next_one_number(&mut self.xml_reader, &mut self.buf) {
            Ok(Some(number)) => {
                return Some(Ok(number));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}
impl<'a, T: std::io::BufRead> SetNumberIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> SetNumberIter<T> {
        SetNumberIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn number_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<SetNumberVector, DeError> {
        let mut device: Option<String> = None;
        let mut name: Option<String> = None;
        let mut state: Option<PropertyState> = None;
        let mut timeout: Option<u32> = None;
        let mut timestamp: Option<DateTime<Utc>> = None;
        let mut message: Option<String> = None;

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Some(attr_value),
                QName(b"name") => name = Some(attr_value),
                QName(b"state") => state = Some(PropertyState::try_from(attr, xml_reader)?),
                QName(b"timeout") => timeout = Some(attr_value.parse::<u32>()?),
                QName(b"timestamp") => {
                    timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?)
                }
                QName(b"message") => message = Some(attr_value),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key.into_inner())?
                    )))
                }
            }
        }
        Ok(SetNumberVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            numbers: Vec::new(),
        })
    }
}

pub struct NewNumberIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for NewNumberIter<'a, T> {
    type Item = Result<OneNumber, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match next_one_number(&mut self.xml_reader, &mut self.buf) {
            Ok(Some(number)) => {
                return Some(Ok(number));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}
impl<'a, T: std::io::BufRead> NewNumberIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> NewNumberIter<T> {
        NewNumberIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn number_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<NewNumberVector, DeError> {
        let mut device: Option<String> = None;
        let mut name: Option<String> = None;
        let mut timestamp: Option<DateTime<Utc>> = None;

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Some(attr_value),
                QName(b"name") => name = Some(attr_value),
                QName(b"timestamp") => {
                    timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?)
                }
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key.into_inner())?
                    )))
                }
            }
        }
        Ok(NewNumberVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            timestamp: timestamp,
            numbers: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn test_set_number() {
        let xml = r#"
    <oneNumber name="SIM_FOCUS_POSITION">
7340
    </oneNumber>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut number_iter = SetNumberIter::new(&mut command_iter);

        let result = number_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            OneNumber {
                name: "SIM_FOCUS_POSITION".to_string(),
                min: None,
                max: None,
                step: None,
                value: 7340.0
            }
        );
    }

    #[test]
    fn test_def_number() {
        let xml = r#"
    <defNumber name="SIM_XRES" label="CCD X resolution" format="%4.0f" min="512" max="8192" step="512">
1280
    </defNumber>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut number_iter = DefNumberIter::new(&mut command_iter);

        let result = number_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            DefNumber {
                name: "SIM_XRES".to_string(),
                label: Some("CCD X resolution".to_string()),
                format: "%4.0f".to_string(),
                min: 512.0,
                max: 8192.0,
                step: 512.0,
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
        let mut command_iter = CommandIter::new(reader);
        let mut number_iter = DefNumberIter::new(&mut command_iter);

        let result = number_iter.next().unwrap().unwrap();
        assert_eq!(
            result,
            DefNumber {
                name: "SIM_XSIZE".to_string(),
                label: Some("CCD X Pixel Size".to_string()),
                format: "%4.2f".to_string(),
                min: 1.0,
                max: 30.0,
                step: 5.0,
                value: 5.2000000000000001776
            }
        );
    }

    #[test]
    fn test_parse_number_normal() {
        let mut buf = Vec::new();
        let xml = r#"-10.505"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let event = reader.read_event_into(&mut buf);
        if let Ok(Event::Text(e)) = event {
            assert_eq!(-10.505, parse_number(&e).unwrap());
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_parse_number_sexagesimal_1() {
        let mut buf = Vec::new();
        let xml = r#"-10 30.3"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let event = reader.read_event_into(&mut buf);
        if let Ok(Event::Text(e)) = event {
            assert_eq!(-10.505, parse_number(&e).unwrap());
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_parse_number_sexagesimal_2() {
        let mut buf = Vec::new();
        let xml = r#"-10:30:18"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let event = reader.read_event_into(&mut buf);
        if let Ok(Event::Text(e)) = event {
            assert_eq!(-10.505, parse_number(&e).unwrap());
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_send_new_number_vector() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

        let command = NewNumberVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            timestamp: Some(timestamp),
            numbers: vec![OneNumber {
                name: String::from_str("seconds").unwrap(),
                max: None,
                min: None,
                step: None,
                value: 3.0,
            }],
        };

        command.send(&mut writer).unwrap();

        let result = writer.into_inner().into_inner();
        assert_eq!(
            String::from_utf8(result).unwrap(),
            String::from_str("<newNumberVector device=\"CCD Simulator\" name=\"Exposure\" timestamp=\"2022-10-13T07:41:56.301\"><oneNumber name=\"seconds\">3</oneNumber></newNumberVector>").unwrap()
        );
    }
}
