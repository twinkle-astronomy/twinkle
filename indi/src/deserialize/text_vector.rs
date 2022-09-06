use quick_xml::events::Event;
use quick_xml::Reader;

use std::str;

use encoding::all::ISO_8859_1;
use encoding::{DecoderTrap, Encoding};

use super::super::*;
use super::*;

pub struct TextIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for TextIter<'a, T> {
    type Item = Result<Text, DeError>;
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
impl<'a, T: std::io::BufRead> TextIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> TextIter<T> {
        TextIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn def_text_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<TextVector, DeError> {
        let mut device: Option<String> = None;
        let mut name: Option<String> = None;
        let mut label: Option<String> = None;
        let mut group: Option<String> = None;
        let mut state: Option<String> = None;
        let mut perm: Option<String> = None;
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
                b"state" => state = Some(attr_value),
                b"perm" => perm = Some(attr_value),
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
        Ok(TextVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            label: label,
            group: group,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            perm: perm.ok_or(DeError::MissingAttr(&"perm"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            texts: HashMap::new(),
        })
    }

    fn next_text(&mut self) -> Result<Option<Text>, DeError> {
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
                        match self.xml_reader.read_event(self.buf) {
                            Ok(Event::Text(e)) => Ok(ISO_8859_1
                                .decode(&e.unescaped()?.into_owned(), DecoderTrap::Strict)?),
                            _ => return Err(DeError::UnexpectedEvent()),
                        };

                    let trailing_event = self.xml_reader.read_event(&mut self.buf)?;
                    match trailing_event {
                        Event::End(_) => (),
                        _ => {
                            return Err(DeError::UnexpectedEvent());
                        }
                    }

                    Ok(Some(Text {
                        name: name?,
                        label: label,
                        value: value?,
                    }))
                }
                tag => Err(DeError::UnexpectedTag(str::from_utf8(tag)?.to_string())),
            },
            Event::End(_) => Ok(None),
            Event::Eof => Ok(None),
            _ => Err(DeError::UnexpectedEvent()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_number() {
        let xml = r#"
    <defText name="ACTIVE_TELESCOPE" label="Telescope">
Telescope Simulator
    </defText>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut number_iter = TextIter::new(&mut command_iter);

        let result = number_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            Text {
                name: "ACTIVE_TELESCOPE".to_string(),
                label: Some("Telescope".to_string()),
                value: "Telescope Simulator".to_string()
            }
        );
    }
}
