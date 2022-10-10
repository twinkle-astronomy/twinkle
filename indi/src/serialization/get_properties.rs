use quick_xml::events::Event;
use quick_xml::Reader;

use std::str;

use super::super::*;
use super::*;

pub struct GetPropertiesIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for GetPropertiesIter<'a, T> {
    type Item = Result<(), DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_get_properties() {
            Ok(Some(m)) => {
                return Some(Ok(m));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}
impl<'a, T: std::io::BufRead> GetPropertiesIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> GetPropertiesIter<T> {
        GetPropertiesIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn get_properties(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<GetProperties, DeError> {
        let mut version: Result<String, DeError> = Err(DeError::MissingAttr(&"version"));
        let mut device: Option<String> = None;
        let mut name: Option<String> = None;

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.unescape_and_decode_value(&xml_reader)?;
            match attr.key {
                b"version" => version = Ok(attr_value),
                b"device" => device = Some(attr_value),
                b"name" => name = Some(attr_value),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key)?
                    )))
                }
            }
        }
        Ok(GetProperties {
            version: version?,
            device: device,
            name: name,
        })
    }

    fn next_get_properties(&mut self) -> Result<Option<()>, DeError> {
        let trailing_event = self.xml_reader.read_event(&mut self.buf)?;
        match trailing_event {
            Event::End(_) => Ok(None),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}
