use quick_xml::events::Event;
use quick_xml::Reader;
use quick_xml::name::QName;

use std::str;

use super::super::*;
use super::*;

pub struct MessageIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for MessageIter<'a, T> {
    type Item = Result<(), DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_message() {
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
impl<'a, T: std::io::BufRead> MessageIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> MessageIter<T> {
        MessageIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn message(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<Message, DeError> {
        let mut device: Option<String> = None;
        let mut timestamp: Option<DateTime<Utc>> = None;
        let mut message: Option<String> = None;

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Some(attr_value),
                QName(b"timestamp") => timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?),
                QName(b"message") => message = Some(attr_value),
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key.into_inner())?
                    )))
                }
            }
        }
        Ok(Message {
            device: device,
            timestamp: timestamp,
            message: message,
        })
    }

    fn next_message(&mut self) -> Result<Option<()>, DeError> {
        let trailing_event = self.xml_reader.read_event_into(&mut self.buf)?;
        match trailing_event {
            Event::End(_) => Ok(None),
            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
        }
    }
}
