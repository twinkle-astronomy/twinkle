use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;

use std::str;

use super::super::*;
use super::*;

pub struct DelPropertyIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for DelPropertyIter<'a, T> {
    type Item = Result<(), DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_del() {
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
impl<'a, T: std::io::BufRead> DelPropertyIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> DelPropertyIter<T> {
        DelPropertyIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn del_property(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<DelProperty, DeError> {
        let mut device: Result<String, DeError> = Err(DeError::MissingAttr(&"device"));
        let mut name: Option<String> = None;
        let mut timestamp: Option<Timestamp> = None;
        let mut message: Option<String> = None;

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Ok(attr_value),
                QName(b"name") => name = Some(attr_value),
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
        Ok(DelProperty {
            device: device?,
            name: name,
            timestamp: timestamp,
            message: message,
        })
    }

    fn next_del(&mut self) -> Result<Option<()>, DeError> {
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

    #[test]
    fn test_get_properties() {
        let xml = r#"
    <delProperty device="Telescope Simulator" name="foothing"/>
                    "#;
        let param: DelProperty = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(param.device, String::from("Telescope Simulator"));
        assert_eq!(param.name, Some(String::from("foothing")));
    }
}