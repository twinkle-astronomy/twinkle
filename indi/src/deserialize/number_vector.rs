use quick_xml::events::Event;
use quick_xml::Reader;

use std::str;

use encoding::all::ISO_8859_1;
use encoding::{DecoderTrap, Encoding};

use super::super::*;
use super::*;

fn next_one_number<T: std::io::BufRead>(
    xml_reader: &mut Reader<T>,
    buf: &mut Vec<u8>,
) -> Result<Option<OneNumber>, DeError> {
    let event = xml_reader.read_event(buf)?;
    match event {
        Event::Start(e) => match e.name() {
            b"oneNumber" => {
                let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));
                let mut min: Option<f64> = None;
                let mut max: Option<f64> = None;
                let mut step: Option<f64> = None;

                for attr in e.attributes() {
                    let attr = attr?;
                    let attr_value = attr.unescape_and_decode_value(&xml_reader)?;

                    match attr.key {
                        b"name" => name = Ok(attr_value),
                        b"min" => min = Some(attr_value.parse::<f64>()?),
                        b"max" => max = Some(attr_value.parse::<f64>()?),
                        b"step" => step = Some(attr_value.parse::<f64>()?),
                        key => {
                            return Err(DeError::UnexpectedAttr(format!(
                                "Unexpected attribute {}",
                                str::from_utf8(key)?
                            )))
                        }
                    }
                }

                let value: Result<f64, DeError> = match xml_reader.read_event(buf) {
                    Ok(Event::Text(e)) => Ok(ISO_8859_1
                        .decode(&e.unescaped()?.into_owned(), DecoderTrap::Strict)?
                        .parse::<f64>()?),
                    e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                };

                let trailing_event = xml_reader.read_event(buf)?;
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
            tag => Err(DeError::UnexpectedTag(str::from_utf8(tag)?.to_string())),
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
            let attr_value = attr.unescape_and_decode_value(&xml_reader)?;
            match attr.key {
                b"device" => device = Some(attr_value),
                b"name" => name = Some(attr_value),
                b"label" => label = Some(attr_value),
                b"group" => group = Some(attr_value),
                b"state" => state = Some(PropertyState::try_from(attr)?),
                b"perm" => perm = Some(PropertyPerm::try_from(attr)?),
                b"timeout" => timeout = Some(attr_value.parse::<u32>()?),
                b"timestamp" => timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?),
                b"message" => message = Some(attr_value),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key)?
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
            numbers: HashMap::new(),
        })
    }

    fn next_number(&mut self) -> Result<Option<DefNumber>, DeError> {
        let event = self.xml_reader.read_event(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                b"defNumber" => {
                    let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));
                    let mut label: Option<String> = None;
                    let mut format: Result<String, DeError> = Err(DeError::MissingAttr(&"format"));
                    let mut min: Result<f64, DeError> = Err(DeError::MissingAttr(&"min"));
                    let mut max: Result<f64, DeError> = Err(DeError::MissingAttr(&"max"));
                    let mut step: Result<f64, DeError> = Err(DeError::MissingAttr(&"step"));

                    for attr in e.attributes() {
                        let attr = attr?;
                        let attr_value = attr.unescape_and_decode_value(&self.xml_reader)?;

                        match attr.key {
                            b"name" => name = Ok(attr_value),
                            b"label" => label = Some(attr_value),
                            b"format" => format = Ok(attr_value),
                            b"min" => min = Ok(attr_value.parse::<f64>()?),
                            b"max" => max = Ok(attr_value.parse::<f64>()?),
                            b"step" => step = Ok(attr_value.parse::<f64>()?),
                            key => {
                                return Err(DeError::UnexpectedAttr(format!(
                                    "Unexpected attribute {}",
                                    str::from_utf8(key)?
                                )))
                            }
                        }
                    }

                    let value: Result<f64, DeError> = match self.xml_reader.read_event(self.buf) {
                        Ok(Event::Text(e)) => Ok(ISO_8859_1
                            .decode(&e.unescaped()?.into_owned(), DecoderTrap::Strict)?
                            .parse::<f64>()?),
                        e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                    };

                    let trailing_event = self.xml_reader.read_event(&mut self.buf)?;
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
                tag => Err(DeError::UnexpectedTag(str::from_utf8(tag)?.to_string())),
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
            let attr_value = attr.unescape_and_decode_value(&xml_reader)?;
            match attr.key {
                b"device" => device = Some(attr_value),
                b"name" => name = Some(attr_value),
                b"state" => state = Some(PropertyState::try_from(attr)?),
                b"timeout" => timeout = Some(attr_value.parse::<u32>()?),
                b"timestamp" => timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?),
                b"message" => message = Some(attr_value),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key)?
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
            numbers: HashMap::new(),
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
            let attr_value = attr.unescape_and_decode_value(&xml_reader)?;
            match attr.key {
                b"device" => device = Some(attr_value),
                b"name" => name = Some(attr_value),
                b"timestamp" => timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key)?
                    )))
                }
            }
        }
        Ok(NewNumberVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            timestamp: timestamp,
            numbers: HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
