use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;

use std::str;

// use encoding::all::ISO_8859_1;
// use encoding::{DecoderTrap, Encoding};

use super::super::*;
use super::*;

impl<'de> Deserialize<'de> for Sexagesimal {
    fn deserialize<D>(deserializer: D) -> Result<Sexagesimal, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        let mut components = s.split([' ', ':']);

        let hour = components
            .next()
            .map(str::parse)
            .transpose()
            .unwrap()
            .unwrap();
        let minute = components.next().map(str::parse).transpose().unwrap();
        let second = components.next().map(str::parse).transpose().unwrap();

        Ok(Sexagesimal {
            hour,
            minute,
            second,
        })
    }
}

impl std::fmt::Display for Sexagesimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hour)?;
        if let Some(minute) = self.minute {
            write!(f, ":{}", minute)?;
        }
        if let Some(second) = self.second {
            write!(f, ":{}", second)?;
        }

        Ok(())
    }
}

impl From<f64> for Sexagesimal {
    fn from(value: f64) -> Self {
        // TODO: try splitting minute and second out of value instead of putting
        //  it all in hour.
        Self {
            hour: value.into(),
            minute: None,
            second: None,
        }
    }
}

impl From<Sexagesimal> for f64 {
    fn from(value: Sexagesimal) -> Self {
        let mut val = value.hour;

        let sign = value.hour.signum();
        let div = 60.0;

        if let Some(minute) = value.minute {
            val += sign * minute / div;
        }
        if let Some(second) = value.second {
            val += sign * second / (div * div);
        }

        val
    }
}

impl CommandtoParam for DefNumberVector {
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_group(&self) -> &Option<String> {
        &self.group
    }
    fn to_param(self, gen: Wrapping<usize>) -> Parameter {
        Parameter::NumberVector(NumberVector {
            gen,
            name: self.name,
            group: self.group,
            label: self.label,
            state: self.state,
            perm: self.perm,
            timeout: self.timeout,
            timestamp: self.timestamp.map(Timestamp::into_inner),
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

    fn update_param(self, param: &mut Parameter) -> Result<String, UpdateError> {
        match param {
            Parameter::NumberVector(number_vector) => {
                number_vector.state = self.state;
                number_vector.timeout = self.timeout;
                number_vector.timestamp = self.timestamp.map(Timestamp::into_inner);
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

impl XmlSerialization for SetOneNumber {
    fn write<'a, T: std::io::Write>(
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

impl XmlSerialization for OneNumber {
    fn write<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let creator = xml_writer
            .create_element("oneNumber")
            .with_attribute(("name", &*self.name));

        creator.write_text_content(BytesText::new(self.value.to_string().as_str()))?;

        Ok(xml_writer)
    }
}

impl XmlSerialization for NewNumberVector {
    fn write<'a, T: std::io::Write>(
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
                    format!("{}", timestamp.deref().format("%Y-%m-%dT%H:%M:%S%.3f")).as_str(),
                ));
            }
            xml_writer = creator.write_inner_content(|xml_writer| {
                for number in self.numbers.iter() {
                    number.write(xml_writer)?;
                }
                Ok(())
            })?;
        }

        Ok(xml_writer)
    }
}

fn parse_number(e: &BytesText) -> Result<Sexagesimal, DeError> {
    let text = &e.unescape()?;
    let val: Sexagesimal = quick_xml::de::from_str(text)?;
    return Ok(val);
}

fn next_set_one_number<T: std::io::BufRead>(
    xml_reader: &mut Reader<T>,
    buf: &mut Vec<u8>,
) -> Result<Option<SetOneNumber>, DeError> {
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

                let value: Result<Sexagesimal, DeError> = match xml_reader.read_event_into(buf) {
                    Ok(Event::Text(e)) => parse_number(&e),
                    e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                };

                let trailing_event = xml_reader.read_event_into(buf)?;
                match trailing_event {
                    Event::End(_) => (),
                    e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                }

                Ok(Some(SetOneNumber {
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
fn next_one_number<T: std::io::BufRead>(
    xml_reader: &mut Reader<T>,
    buf: &mut Vec<u8>,
) -> Result<Option<OneNumber>, DeError> {
    let event = xml_reader.read_event_into(buf)?;
    match event {
        Event::Start(e) => match e.name() {
            QName(b"oneNumber") => {
                let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));

                for attr in e.attributes() {
                    let attr = attr?;
                    let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();

                    match attr.key {
                        QName(b"name") => name = Ok(attr_value),
                        key => {
                            return Err(DeError::UnexpectedAttr(format!(
                                "Unexpected attribute {}",
                                str::from_utf8(key.into_inner())?
                            )))
                        }
                    }
                }

                let value: Result<Sexagesimal, DeError> = match xml_reader.read_event_into(buf) {
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
        let mut timestamp: Option<Timestamp> = None;
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
                    timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?.into())
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

                    let value: Result<Sexagesimal, DeError> =
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
    type Item = Result<SetOneNumber, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match next_set_one_number(&mut self.xml_reader, &mut self.buf) {
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
        let mut timestamp: Option<Timestamp> = None;
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
                    timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?.into())
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
            timeout,
            timestamp,
            message,
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
        let mut timestamp: Option<Timestamp> = None;

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Some(attr_value),
                QName(b"name") => name = Some(attr_value),
                QName(b"timestamp") => {
                    timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?.into())
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
    fn test_def_number() {
        let xml = r#"
        <defNumber name="SIM_XRES" label="CCD X resolution" format="%4.0f" min="512" max="8192" step="512">
    1280
        </defNumber>
                    "#;
        let command: Result<DefNumber, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.name, "SIM_XRES");
                assert_eq!(param.label, Some(String::from("CCD X resolution")));
                assert_eq!(param.value, 1280.0.into());
            }
            Err(e) => {
                panic!("Unexpected: {:?}", e);
            }
        }
    }

    #[test]
    fn test_def_number_vector() {
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
        let command: Result<DefNumberVector, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "SIMULATOR_SETTINGS");
                assert_eq!(param.numbers.len(), 3)
            }
            e => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_set_number_vector() {
        let xml = r#"
    <setNumberVector device="CCD Simulator" name="SIM_FOCUSING" state="Ok" timeout="60" timestamp="2022-10-01T21:21:10">
    <oneNumber name="SIM_FOCUS_POSITION">
    7340
    </oneNumber>
    <oneNumber name="SIM_FOCUS_MAX">
    100000
    </oneNumber>
    <oneNumber name="SIM_SEEING">
    3.5
    </oneNumber>
    </setNumberVector>
"#;

        let command: Result<SetNumberVector, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "SIM_FOCUSING");
                assert_eq!(param.numbers.len(), 3)
            }
            e => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_new_number_vector() {
        let xml = r#"
    <newNumberVector device="CCD Simulator" name="SIM_FOCUSING" timestamp="2022-10-01T21:21:10">
    <oneNumber name="SIM_FOCUS_POSITION">
    7340
    </oneNumber>
    <oneNumber name="SIM_FOCUS_MAX">
    100000
    </oneNumber>
    <oneNumber name="SIM_SEEING">
    3.5
    </oneNumber>
    </newNumberVector>
    "#;

        let command: Result<NewNumberVector, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "SIM_FOCUSING");
                assert_eq!(param.numbers.len(), 3)
            }
            e => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_parse_number_normal() {
        let xml = r#"-10.505"#;

        let event: Result<Sexagesimal, _> = quick_xml::de::from_str(xml);

        if let Ok(e) = event {
            assert_eq!(Into::<Sexagesimal>::into(-10.505), e);
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_parse_number_sexagesimal_1() {
        let xml = r#"-10 30.3"#;

        let event: Result<Sexagesimal, _> = quick_xml::de::from_str(xml);

        if let Ok(e) = event {
            assert_eq!(-10.505, e.into());
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_parse_number_sexagesimal_2() {
        let xml = r#"-10:30:18"#;

        let event: Result<Sexagesimal, _> = quick_xml::de::from_str(xml);

        if let Ok(e) = event {
            assert_eq!(-10.505, e.into());
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_send_new_number_vector() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z")
            .unwrap()
            .into();

        let command = NewNumberVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            timestamp: Some(timestamp),
            numbers: vec![OneNumber {
                name: String::from_str("seconds").unwrap(),
                value: 3.0.into(),
            }],
        };

        command.write(&mut writer).unwrap();

        let result = writer.into_inner().into_inner();
        assert_eq!(
            String::from_utf8(result).unwrap(),
            String::from_str("<newNumberVector device=\"CCD Simulator\" name=\"Exposure\" timestamp=\"2022-10-13T07:41:56.301\"><oneNumber name=\"seconds\">3</oneNumber></newNumberVector>").unwrap()
        );
    }
}
