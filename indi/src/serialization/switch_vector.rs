use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;

use std::str;

use super::super::*;
use super::*;

impl CommandtoParam for DefSwitchVector {
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_group(&self) -> &Option<String> {
        &self.group
    }
    fn to_param(self, gen: Wrapping<usize>) -> Parameter {
        Parameter::SwitchVector(SwitchVector {
            gen,
            name: self.name,
            group: self.group,
            label: self.label,
            state: self.state,
            perm: self.perm,
            rule: self.rule,
            timeout: self.timeout,
            timestamp: self.timestamp,
            values: self
                .switches
                .into_iter()
                .map(|i| {
                    (
                        i.name,
                        Switch {
                            label: i.label,
                            value: i.value,
                        },
                    )
                })
                .collect(),
        })
    }
}

impl CommandToUpdate for SetSwitchVector {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn update_param(self, param: &mut Parameter) -> Result<String, UpdateError> {
        match param {
            Parameter::SwitchVector(switch_vector) => {
                switch_vector.timestamp = self.timestamp;
                for switch in self.switches {
                    if let Some(existing) = switch_vector.values.get_mut(&switch.name) {
                        existing.value = switch.value;
                    }
                }
                Ok(self.name)
            }
            _ => Err(UpdateError::ParameterTypeMismatch(self.name.clone())),
        }
    }
}

impl XmlSerialization for OneSwitch {
    fn write<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let creator = xml_writer
            .create_element("oneSwitch")
            .with_attribute(("name", &*self.name));

        match self.value {
            SwitchState::On => creator.write_text_content(BytesText::new("On")),
            SwitchState::Off => creator.write_text_content(BytesText::new("Off")),
        }?;

        Ok(xml_writer)
    }
}

impl XmlSerialization for NewSwitchVector {
    fn write<'a, T: std::io::Write>(
        &self,
        mut xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        {
            let mut creator = xml_writer
                .create_element("newSwitchVector")
                .with_attribute(("device", &*self.device))
                .with_attribute(("name", &*self.name));

            if let Some(timestamp) = &self.timestamp {
                creator = creator.with_attribute((
                    "timestamp",
                    format!("{}", timestamp.format("%Y-%m-%dT%H:%M:%S%.3f")).as_str(),
                ));
            }
            xml_writer = creator.write_inner_content(|xml_writer| {
                for number in self.switches.iter() {
                    number.write(xml_writer)?;
                }
                Ok(())
            })?;
        }

        Ok(xml_writer)
    }
}

fn next_one_switch<T: std::io::BufRead>(
    xml_reader: &mut Reader<T>,
    buf: &mut Vec<u8>,
) -> Result<Option<OneSwitch>, DeError> {
    let event = xml_reader.read_event_into(buf)?;
    match event {
        Event::Start(e) => match e.name() {
            QName(b"oneSwitch") => {
                let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));

                for attr in e.attributes() {
                    let attr = attr?;
                    let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();

                    match attr.key {
                        QName(b"name") => name = Ok(attr_value),
                        key => {
                            return Err(DeError::UnexpectedAttr(format!(
                                "Unexpected attribute {}",
                                str::from_utf8(key.into_inner())?
                            )))
                        }
                    }
                }

                let value: Result<SwitchState, DeError> = match xml_reader.read_event_into(buf) {
                    Ok(Event::Text(e)) => SwitchState::try_from_event(e),
                    e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                };

                let trailing_event = xml_reader.read_event_into(buf)?;
                match trailing_event {
                    Event::End(_) => (),
                    e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                }

                Ok(Some(OneSwitch {
                    name: name?,
                    value: value?,
                }))
            }
            tag => Err(DeError::UnexpectedTag(
                str::from_utf8(tag.into_inner())?.to_string(),
            )),
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
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Some(attr_value),
                QName(b"name") => name = Some(attr_value),
                QName(b"label") => label = Some(attr_value),
                QName(b"group") => group = Some(attr_value),
                QName(b"state") => state = Some(PropertyState::try_from(attr, xml_reader)?),
                QName(b"perm") => perm = Some(PropertyPerm::try_from(attr, xml_reader)?),
                QName(b"rule") => rule = Some(SwitchRule::try_from(attr, xml_reader)?),
                QName(b"timeout") => timeout = Some(attr_value.parse::<u32>()?),
                QName(b"timestamp") => {
                    timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?)
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
            switches: Vec::new(),
        })
    }

    fn next_switch(&mut self) -> Result<Option<DefSwitch>, DeError> {
        let event = self.xml_reader.read_event_into(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                QName(b"defSwitch") => {
                    let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));
                    let mut label: Option<String> = None;

                    for attr in e.attributes() {
                        let attr = attr?;
                        let attr_value = attr
                            .decode_and_unescape_value(self.xml_reader)?
                            .into_owned();

                        match attr.key {
                            QName(b"name") => name = Ok(attr_value),
                            QName(b"label") => label = Some(attr_value),
                            key => {
                                return Err(DeError::UnexpectedAttr(format!(
                                    "Unexpected attribute {}",
                                    str::from_utf8(key.into_inner())?
                                )))
                            }
                        }
                    }

                    let value: Result<SwitchState, DeError> =
                        match self.xml_reader.read_event_into(self.buf) {
                            Ok(Event::Text(e)) => SwitchState::try_from_event(e),
                            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                        };

                    let trailing_event = self.xml_reader.read_event_into(&mut self.buf)?;
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
                tag => Err(DeError::UnexpectedTag(
                    str::from_utf8(tag.into_inner())?.to_string(),
                )),
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
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Some(attr_value),
                QName(b"name") => name = Some(attr_value),
                QName(b"state") => state = Some(PropertyState::try_from(attr, xml_reader)?),
                QName(b"timeout") => timeout = Some(attr_value.parse::<u32>()?),
                QName(b"timestamp") => {
                    timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?)
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
        Ok(SetSwitchVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            timeout: timeout,
            timestamp: timestamp,
            message: message,
            switches: Vec::new(),
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
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Some(attr_value),
                QName(b"name") => name = Some(attr_value),
                QName(b"timestamp") => {
                    timestamp = Some(DateTime::from_str(&format!("{}Z", &attr_value))?)
                }
                key => {
                    return Err(DeError::UnexpectedAttr(format!(
                        "Unexpected attribute {}",
                        str::from_utf8(key.into_inner())?
                    )))
                }
            }
        }
        Ok(NewSwitchVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            timestamp: timestamp,
            switches: Vec::new(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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
    #[test]
    fn test_send_new_switch_vector() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

        let command = NewSwitchVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            timestamp: Some(timestamp),
            switches: vec![OneSwitch {
                name: String::from_str("seconds").unwrap(),
                value: SwitchState::On,
            }],
        };

        command.write(&mut writer).unwrap();

        let result = writer.into_inner().into_inner();
        assert_eq!(
            String::from_utf8(result).unwrap(),
            String::from_str("<newSwitchVector device=\"CCD Simulator\" name=\"Exposure\" timestamp=\"2022-10-13T07:41:56.301\"><oneSwitch name=\"seconds\">On</oneSwitch></newSwitchVector>").unwrap()
        );
    }
}
