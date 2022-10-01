use quick_xml::events::Event;
use quick_xml::Reader;

use std::str;

use super::super::*;
use super::*;

pub struct DefLightIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for DefLightIter<'a, T> {
    type Item = Result<DefLight, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_light() {
            Ok(Some(light)) => {
                return Some(Ok(light));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}
impl<'a, T: std::io::BufRead> DefLightIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> DefLightIter<T> {
        DefLightIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn light_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<DefLightVector, DeError> {
        let mut device: Option<String> = None;
        let mut name: Option<String> = None;
        let mut label: Option<String> = None;
        let mut group: Option<String> = None;
        let mut state: Option<PropertyState> = None;
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
        Ok(DefLightVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            label: label,
            group: group,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            timestamp: timestamp,
            message: message,
            lights: HashMap::new(),
        })
    }

    fn next_light(&mut self) -> Result<Option<DefLight>, DeError> {
        let event = self.xml_reader.read_event(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                b"defLight" => {
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

                    let value: Result<PropertyState, DeError> =
                        match self.xml_reader.read_event(self.buf) {
                            Ok(Event::Text(e)) => PropertyState::try_from(e),
                            _ => return Err(DeError::UnexpectedEvent()),
                        };

                    let trailing_event = self.xml_reader.read_event(&mut self.buf)?;
                    match trailing_event {
                        Event::End(_) => (),
                        _ => {
                            return Err(DeError::UnexpectedEvent());
                        }
                    }

                    Ok(Some(DefLight {
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

pub struct SetLightIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for SetLightIter<'a, T> {
    type Item = Result<OneLight, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_light() {
            Ok(Some(light)) => {
                return Some(Ok(light));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}
impl<'a, T: std::io::BufRead> SetLightIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> SetLightIter<T> {
        SetLightIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn light_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<SetLightVector, DeError> {
        let mut device: Option<String> = None;
        let mut name: Option<String> = None;
        let mut state: Option<PropertyState> = None;
        let mut timestamp: Option<DateTime<Utc>> = None;
        let mut message: Option<String> = None;

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.unescape_and_decode_value(&xml_reader)?;
            match attr.key {
                b"device" => device = Some(attr_value),
                b"name" => name = Some(attr_value),
                b"state" => state = Some(PropertyState::try_from(attr)?),
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
        Ok(SetLightVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            timestamp: timestamp,
            message: message,
            lights: HashMap::new(),
        })
    }

    fn next_light(&mut self) -> Result<Option<OneLight>, DeError> {
        let event = self.xml_reader.read_event(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                b"oneLight" => {
                    let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));

                    for attr in e.attributes() {
                        let attr = attr?;
                        let attr_value = attr.unescape_and_decode_value(&self.xml_reader)?;

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

                    let value: Result<PropertyState, DeError> =
                        match self.xml_reader.read_event(self.buf) {
                            Ok(Event::Text(e)) => PropertyState::try_from(e),
                            _ => return Err(DeError::UnexpectedEvent()),
                        };

                    let trailing_event = self.xml_reader.read_event(&mut self.buf)?;
                    match trailing_event {
                        Event::End(_) => (),
                        _ => {
                            return Err(DeError::UnexpectedEvent());
                        }
                    }

                    Ok(Some(OneLight {
                        name: name?,
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
    fn test_parse_light() {
        let xml = r#"
    <defLight name="INDI_DISABLED" label="Disabled">
Ok
    </defLight>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut light_iter = DefLightIter::new(&mut command_iter);

        let result = light_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            DefLight {
                name: "INDI_DISABLED".to_string(),
                label: Some("Disabled".to_string()),
                value: PropertyState::Ok
            }
        );

        let xml = r#"
    <defLight name="INDI_DISABLED" label="Disabled">
Busy
    </defLight>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut light_iter = DefLightIter::new(&mut command_iter);

        let result = light_iter.next().unwrap().unwrap();
        assert_eq!(
            result,
            DefLight {
                name: "INDI_DISABLED".to_string(),
                label: Some("Disabled".to_string()),
                value: PropertyState::Busy
            }
        );
    }

    #[test]
    fn test_set_parse_light() {
        let xml = r#"
    <oneLight name="INDI_DISABLED" >
Ok
    </oneLight>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut light_iter = SetLightIter::new(&mut command_iter);

        let result = light_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            OneLight {
                name: "INDI_DISABLED".to_string(),
                value: PropertyState::Ok
            }
        );
    }
}
