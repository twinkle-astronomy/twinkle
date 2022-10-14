use quick_xml::events::Event;
use quick_xml::Reader;
use std::str;

use encoding::all::ISO_8859_1;
use encoding::{DecoderTrap, EncoderTrap, Encoding};

use log::warn;

use super::super::*;
use super::*;

impl XmlSerialization for OneText {
    fn send<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let creator = xml_writer
            .create_element("oneText")
            .with_attribute(("name", &*self.name));

        match ISO_8859_1.encode(self.value.as_str(), EncoderTrap::Replace) {
            Ok(content) => {
                creator.write_text_content(BytesText::from_plain(&content))?;
            }
            Err(e) => {
                warn!(
                    "Encoding error during indi::XmlSerialization::send(): {:?}",
                    e
                );
            }
        }

        Ok(xml_writer)
    }
}

impl XmlSerialization for NewTextVector {
    fn send<'a, T: std::io::Write>(
        &self,
        mut xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        {
            let mut creator = xml_writer
                .create_element("newTextVector")
                .with_attribute(("device", &*self.device))
                .with_attribute(("name", &*self.name));

            if let Some(timestamp) = &self.timestamp {
                creator = creator.with_attribute((
                    "timestamp",
                    format!("{}", timestamp.format("%Y-%m-%dT%H:%M:%S%.3f")).as_str(),
                ));
            }
            xml_writer = creator.write_inner_content(|xml_writer| {
                for text in self.texts.iter() {
                    text.send(xml_writer)?;
                }
                Ok(())
            })?;
        }

        Ok(xml_writer)
    }
}

fn next_one_text<T: std::io::BufRead>(
    xml_reader: &mut Reader<T>,
    buf: &mut Vec<u8>,
) -> Result<Option<OneText>, DeError> {
    let event = xml_reader.read_event(buf)?;
    match event {
        Event::Start(e) => match e.name() {
            b"oneText" => {
                let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));

                for attr in e.attributes() {
                    let attr = attr?;
                    let attr_value = attr.unescape_and_decode_value(&xml_reader)?;

                    match attr.key {
                        b"name" => name = Ok(attr_value),
                        key => {
                            return Err(DeError::UnexpectedAttr(format!(
                                "Unexpected attribute {}",
                                str::from_utf8(key)?
                            )))
                        }
                    }
                }

                let value: Result<String, DeError> = match xml_reader.read_event(buf)? {
                    Event::Text(e) => {
                        let v =
                            ISO_8859_1.decode(&e.unescaped()?.into_owned(), DecoderTrap::Strict)?;
                        match xml_reader.read_event(buf)? {
                            Event::End(_) => Ok(v),
                            e => Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                        }
                    }
                    Event::End(_) => Ok(String::from("")),
                    e => Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                };

                Ok(Some(OneText {
                    name: name?,
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

pub struct NewTextIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for NewTextIter<'a, T> {
    type Item = Result<OneText, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match next_one_text(&mut self.xml_reader, &mut self.buf) {
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

impl<'a, T: std::io::BufRead> NewTextIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> NewTextIter<T> {
        NewTextIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn text_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<NewTextVector, DeError> {
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
        Ok(NewTextVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            timestamp: timestamp,
            texts: Vec::new(),
        })
    }
}

pub struct SetTextIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for SetTextIter<'a, T> {
    type Item = Result<OneText, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match next_one_text(&mut self.xml_reader, &mut self.buf) {
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

impl<'a, T: std::io::BufRead> SetTextIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> SetTextIter<T> {
        SetTextIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn text_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<SetTextVector, DeError> {
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
        Ok(SetTextVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            texts: Vec::new(),
        })
    }
}

pub struct DefTextIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for DefTextIter<'a, T> {
    type Item = Result<DefText, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_text() {
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

impl<'a, T: std::io::BufRead> DefTextIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> DefTextIter<T> {
        DefTextIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn text_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<DefTextVector, DeError> {
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
        Ok(DefTextVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            label: label,
            group: group,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            perm: perm.ok_or(DeError::MissingAttr(&"perm"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            texts: Vec::new(),
        })
    }
    fn next_text(&mut self) -> Result<Option<DefText>, DeError> {
        let event = self.xml_reader.read_event(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                b"defText" => {
                    let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));
                    let mut label: Option<String> = None;

                    for attr in e.attributes() {
                        let attr = attr?;
                        let attr_value = attr.unescape_and_decode_value(&self.xml_reader)?;

                        match attr.key {
                            b"name" => name = Ok(attr_value),
                            b"label" => label = Some(attr_value),
                            key => {
                                return Err(DeError::UnexpectedAttr(format!(
                                    "Unexpected attribute {}",
                                    str::from_utf8(key)?
                                )))
                            }
                        }
                    }

                    let value: Result<String, DeError> =
                        match self.xml_reader.read_event(self.buf)? {
                            Event::Text(e) => {
                                let v = ISO_8859_1
                                    .decode(&e.unescaped()?.into_owned(), DecoderTrap::Strict)?;
                                match self.xml_reader.read_event(&mut self.buf)? {
                                    Event::End(_) => Ok(v),
                                    e => Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                                }
                            }
                            Event::End(_) => Ok(String::from("")),
                            e => Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                        };

                    Ok(Some(DefText {
                        name: name?,
                        label: label,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_def_text() {
        let xml = r#"
    <defText name="ACTIVE_TELESCOPE" label="Telescope">
Telescope Simulator
    </defText>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut number_iter = DefTextIter::new(&mut command_iter);

        let result = number_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            DefText {
                name: "ACTIVE_TELESCOPE".to_string(),
                label: Some("Telescope".to_string()),
                value: "Telescope Simulator".to_string()
            }
        );
    }

    #[test]
    fn test_one_text() {
        let xml = b"\
    <oneText name=\"ACTIVE_TELESCOPE\">
Simulator \xFF changed
    </oneText>
";

        let mut reader = Reader::from_bytes(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut number_iter = SetTextIter::new(&mut command_iter);

        let result = number_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            OneText {
                name: "ACTIVE_TELESCOPE".to_string(),
                value: "Simulator ÿ changed".to_string()
            }
        );
    }

    #[test]
    fn test_send_new_text_vector() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

        let command = NewTextVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            timestamp: Some(timestamp),
            texts: vec![OneText {
                name: String::from_str("seconds").unwrap(),
                value: String::from_str("Long ÿ enough").unwrap(),
            }],
        };

        command.send(&mut writer).unwrap();

        let result = writer.into_inner().into_inner();
        assert_eq!(
            result,
            b"<newTextVector device=\"CCD Simulator\" name=\"Exposure\" timestamp=\"2022-10-13T07:41:56.301\"><oneText name=\"seconds\">Long \xFF enough</oneText></newTextVector>"
        );
    }
}
