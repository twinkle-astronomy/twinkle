use quick_xml::events::Event;
use quick_xml::Reader;

use std::str;

use super::super::*;
use super::*;

fn next_one_switch<T: std::io::BufRead>(
    xml_reader: &mut Reader<T>,
    buf: &mut Vec<u8>,
) -> Result<Option<OneSwitch>, DeError> {
    let event = xml_reader.read_event(buf)?;
    match event {
        Event::Start(e) => match e.name() {
            b"oneSwitch" => {
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

                let value: Result<SwitchState, DeError> = match xml_reader.read_event(buf) {
                    Ok(Event::Text(e)) => SwitchState::try_from(e),
                    e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                };

                let trailing_event = xml_reader.read_event(buf)?;
                match trailing_event {
                    Event::End(_) => (),
                    e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                }

                Ok(Some(OneSwitch {
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

pub struct DefSwitchIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for DefSwitchIter<'a, T> {
    type Item = Result<DefSwitch, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_switch() {
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
impl<'a, T: std::io::BufRead> DefSwitchIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> DefSwitchIter<T> {
        DefSwitchIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn switch_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<DefSwitchVector, DeError> {
        let mut device: Option<String> = None;
        let mut name: Option<String> = None;
        let mut label: Option<String> = None;
        let mut group: Option<String> = None;
        let mut state: Option<PropertyState> = None;
        let mut perm: Option<PropertyPerm> = None;
        let mut rule: Option<SwitchRule> = None;
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
                b"rule" => rule = Some(SwitchRule::try_from(attr)?),
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
        Ok(DefSwitchVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            label: label,
            group: group,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            perm: perm.ok_or(DeError::MissingAttr(&"perm"))?,
            rule: rule.ok_or(DeError::MissingAttr(&"perm"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            switches: HashMap::new(),
        })
    }

    fn next_switch(&mut self) -> Result<Option<DefSwitch>, DeError> {
        let event = self.xml_reader.read_event(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                b"defSwitch" => {
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

                    let value: Result<SwitchState, DeError> =
                        match self.xml_reader.read_event(self.buf) {
                            Ok(Event::Text(e)) => SwitchState::try_from(e),
                            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                        };

                    let trailing_event = self.xml_reader.read_event(&mut self.buf)?;
                    match trailing_event {
                        Event::End(_) => (),
                        e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                    }

                    Ok(Some(DefSwitch {
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

pub struct SetSwitchIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for SetSwitchIter<'a, T> {
    type Item = Result<OneSwitch, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match next_one_switch(&mut self.xml_reader, &mut self.buf) {
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

impl<'a, T: std::io::BufRead> SetSwitchIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> SetSwitchIter<T> {
        SetSwitchIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn switch_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<SetSwitchVector, DeError> {
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
        Ok(SetSwitchVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            switches: HashMap::new(),
        })
    }
}

pub struct NewSwitchIter<'a, T: std::io::BufRead> {
    xml_reader: &'a mut Reader<T>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: std::io::BufRead> Iterator for NewSwitchIter<'a, T> {
    type Item = Result<OneSwitch, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match next_one_switch(&mut self.xml_reader, &mut self.buf) {
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

impl<'a, T: std::io::BufRead> NewSwitchIter<'a, T> {
    pub fn new(command_iter: &mut CommandIter<T>) -> NewSwitchIter<T> {
        NewSwitchIter {
            xml_reader: &mut command_iter.xml_reader,
            buf: &mut command_iter.buf,
        }
    }

    pub fn switch_vector(
        xml_reader: &Reader<T>,
        start_event: &events::BytesStart,
    ) -> Result<NewSwitchVector, DeError> {
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
        Ok(NewSwitchVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            timestamp: timestamp,
            switches: HashMap::new(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_switch() {
        let xml = r#"
    <defSwitch name="INDI_DISABLED" label="Disabled">
On
    </defSwitch>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut switch_iter = DefSwitchIter::new(&mut command_iter);

        let result = switch_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            DefSwitch {
                name: "INDI_DISABLED".to_string(),
                label: Some("Disabled".to_string()),
                value: SwitchState::On
            }
        );

        let xml = r#"
    <defSwitch name="INDI_DISABLED" label="Disabled">
Off
    </defSwitch>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut switch_iter = DefSwitchIter::new(&mut command_iter);

        let result = switch_iter.next().unwrap().unwrap();
        assert_eq!(
            result,
            DefSwitch {
                name: "INDI_DISABLED".to_string(),
                label: Some("Disabled".to_string()),
                value: SwitchState::Off
            }
        );
    }

    #[test]
    fn test_one_switch() {
        let xml = r#"
    <oneSwitch name="INDI_DISABLED">
On
    </oneSwitch>
"#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);
        let mut switch_iter = SetSwitchIter::new(&mut command_iter);

        let result = switch_iter.next().unwrap().unwrap();

        assert_eq!(
            result,
            OneSwitch {
                name: "INDI_DISABLED".to_string(),
                value: SwitchState::On
            }
        );
    }
}
