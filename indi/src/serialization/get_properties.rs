use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::{Reader, Writer};

use std::str;

use super::super::*;
use super::*;

impl XmlSerialization for GetProperties {
    fn write<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let mut creator = xml_writer
            .create_element("getProperties")
            .with_attribute(("version", &*self.version));

        if let Some(device) = &self.device {
            creator = creator.with_attribute(("device", &device[..]));
        }
        if let Some(name) = &self.name {
            creator = creator.with_attribute(("name", &name[..]));
        }

        creator.write_empty()?;
        Ok(xml_writer)
    }
}

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
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"version") => version = Ok(attr_value),
                QName(b"device") => device = Some(attr_value),
                QName(b"name") => name = Some(attr_value),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key.into_inner())?
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
        let trailing_event = self.xml_reader.read_event_into(&mut self.buf)?;
        match trailing_event {
            Event::End(_) => Ok(None),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_send_get_properties() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        let command = GetProperties {
            version: String::from_str("1.7").unwrap(),
            device: Some(String::from_str("CCD Simulator").unwrap()),
            name: None,
        };

        command.write(&mut writer).unwrap();

        let result = writer.into_inner().into_inner();
        assert_eq!(
            String::from_utf8(result).unwrap(),
            String::from_str("<getProperties version=\"1.7\" device=\"CCD Simulator\"/>").unwrap()
        );
    }
}
