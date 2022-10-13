use quick_xml::events::Event;
use quick_xml::Reader;

use std::str;

use super::super::*;
use super::*;

impl XmlSerialization for EnableBlob {
    fn send<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let mut creator = xml_writer
            .create_element("enableBLOB")
            .with_attribute(("device", &*self.device));

        if let Some(name) = &self.name {
            creator = creator.with_attribute(("name", &name[..]));
        }

        match self.enabled {
            BlobEnable::Never => creator.write_text_content(BytesText::from_plain_str("Never")),
            BlobEnable::Also => creator.write_text_content(BytesText::from_plain_str("Also")),
            BlobEnable::Only => creator.write_text_content(BytesText::from_plain_str("Only")),
        }?;

        xml_writer.inner().flush()?;
        Ok(xml_writer)
    }
}

pub struct DefBlobIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for DefBlobIter<'a, T> {
    type Item = Result<DefBlob, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_blob() {
            Ok(Some(switch)) => {
                return Some(Ok(switch));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}
impl<'a, T: std::io::BufRead> DefBlobIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> DefBlobIter<T> {
        DefBlobIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn blob_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<DefBlobVector, DeError> {
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
        Ok(DefBlobVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            label: label,
            group: group,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            perm: perm.ok_or(DeError::MissingAttr(&"perm"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            blobs: Vec::new(),
        })
    }

    fn next_blob(&mut self) -> Result<Option<DefBlob>, DeError> {
        let event = self.xml_reader.read_event(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                b"defBLOB" => {
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

                    let trailing_event = self.xml_reader.read_event(&mut self.buf)?;
                    match trailing_event {
                        Event::End(_) => (),
                        e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                    }

                    Ok(Some(DefBlob {
                        name: name?,
                        label: label,
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

pub struct SetBlobIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for SetBlobIter<'a, T> {
    type Item = Result<OneBlob, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_blob() {
            Ok(Some(switch)) => {
                return Some(Ok(switch));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}
impl<'a, T: std::io::BufRead> SetBlobIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> SetBlobIter<T> {
        SetBlobIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn blob_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<SetBlobVector, DeError> {
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
        Ok(SetBlobVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            blobs: Vec::new(),
        })
    }

    fn next_blob(&mut self) -> Result<Option<OneBlob>, DeError> {
        let event = self.xml_reader.read_event(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                b"oneBLOB" => {
                    let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));
                    let mut size: Result<u64, DeError> = Err(DeError::MissingAttr(&"size"));
                    let mut enclen: Option<u64> = None;
                    let mut format: Result<String, DeError> = Err(DeError::MissingAttr(&"format"));

                    for attr in e.attributes() {
                        let attr = attr?;
                        let attr_value = attr.unescape_and_decode_value(&self.xml_reader)?;

                        match attr.key {
                            b"name" => name = Ok(attr_value),
                            b"format" => format = Ok(attr_value),
                            b"size" => size = Ok(attr_value.parse::<u64>()?),
                            b"enclen" => enclen = Some(attr_value.parse::<u64>()?),
                            key => {
                                return Err(DeError::UnexpectedAttr(format!(
                                    "Unexpected attribute {}",
                                    str::from_utf8(key)?
                                )))
                            }
                        }
                    }

                    let value: Result<Vec<u8>, DeError> = match self.xml_reader.read_event(self.buf)
                    {
                        Ok(Event::Text(e)) => match size {
                            Ok(size) => {
                                let mut result = Vec::with_capacity(size.try_into().unwrap());
                                let esc = e.unescaped()?;

                                for line in esc.split(|b| *b == b'\n') {
                                    base64::decode_config_buf(line, base64::STANDARD, &mut result)
                                        .unwrap();
                                }

                                Ok(result)
                            }
                            Err(_) => Err(DeError::MissingAttr(&"size")),
                        },
                        e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                    };

                    let trailing_event = self.xml_reader.read_event(&mut self.buf)?;
                    match trailing_event {
                        Event::End(_) => (),
                        e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                    }

                    Ok(Some(OneBlob {
                        name: name?,
                        size: size?,
                        enclen: enclen,
                        format: format?,
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
    fn test_parse_blob() {
        let xml = r#"
    <defBLOB name="INDI_DISABLED" label="Disabled"/>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        reader.expand_empty_elements(true);
        let mut command_iter = CommandIter::new(reader);
        let mut switch_iter = DefBlobIter::new(&mut command_iter);

        let result = switch_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            DefBlob {
                name: "INDI_DISABLED".to_string(),
                label: Some("Disabled".to_string()),
            }
        );

        let xml = r#"
    <defBLOB name="INDI_DISABLED" label="Disabled"/>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        reader.expand_empty_elements(true);
        let mut command_iter = CommandIter::new(reader);
        let mut switch_iter = DefBlobIter::new(&mut command_iter);

        let result = switch_iter.next().unwrap().unwrap();
        assert_eq!(
            result,
            DefBlob {
                name: "INDI_DISABLED".to_string(),
                label: Some("Disabled".to_string()),
            }
        );
    }

    #[test]
    fn test_send_enable_blob() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        let command = EnableBlob {
            device: String::from_str("CCD Simulator").unwrap(),
            name: None,
            enabled: BlobEnable::Also,
        };

        command.send(&mut writer).unwrap();

        let result = writer.into_inner().into_inner();
        assert_eq!(
            String::from_utf8(result).unwrap(),
            String::from_str("<enableBLOB device=\"CCD Simulator\">Also</enableBLOB>").unwrap()
        );
    }

    #[test]
    fn test_set_blob() {
        let xml = include_str!("../../tests/image_capture_one_blob.log");

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        reader.expand_empty_elements(true);
        let mut command_iter = CommandIter::new(reader);
        let mut switch_iter = SetBlobIter::new(&mut command_iter);

        let result = switch_iter.next().unwrap().unwrap();

        assert_eq!(result.name, "CCD1".to_string());
        assert_eq!(result.size, 23040);
        assert_eq!(result.enclen, Some(30720));
        assert_eq!(result.format, ".fits");
        assert_eq!(result.value.len(), 23040);
    }
}
