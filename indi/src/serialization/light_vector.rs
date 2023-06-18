use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;

use std::str;

use super::super::*;
use super::*;

impl CommandtoParam for DefLightVector {
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_group(&self) -> &Option<String> {
        &self.group
    }
    fn to_param(self, gen: Wrapping<usize>) -> Parameter {
        Parameter::LightVector(LightVector {
            gen,
            name: self.name,
            group: self.group,
            label: self.label,
            state: self.state,
            timestamp: self.timestamp.map(Timestamp::into_inner),
            values: self
                .lights
                .into_iter()
                .map(|i| {
                    (
                        i.name,
                        Light {
                            label: i.label,
                            value: i.value,
                        },
                    )
                })
                .collect(),
        })
    }
}

impl CommandToUpdate for SetLightVector {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn update_param(self, param: &mut Parameter) -> Result<String, UpdateError> {
        match param {
            Parameter::LightVector(light_vector) => {
                light_vector.state = self.state;
                light_vector.timestamp = self.timestamp.map(Timestamp::into_inner);
                for light in self.lights {
                    if let Some(existing) = light_vector.values.get_mut(&light.name) {
                        existing.value = light.value;
                    }
                }
                Ok(self.name)
            }
            _ => Err(UpdateError::ParameterTypeMismatch(self.name.clone())),
        }
    }
}

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
        let mut timestamp: Option<Timestamp> = None;
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
        Ok(DefLightVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            label: label,
            group: group,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            timestamp: timestamp,
            message: message,
            lights: Vec::new(),
        })
    }

    fn next_light(&mut self) -> Result<Option<DefLight>, DeError> {
        let event = self.xml_reader.read_event_into(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                QName(b"defLight") => {
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

                    let value: Result<PropertyState, DeError> =
                        match self.xml_reader.read_event_into(self.buf) {
                            Ok(Event::Text(e)) => PropertyState::try_from_event(e),
                            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                        };

                    let trailing_event = self.xml_reader.read_event_into(&mut self.buf)?;
                    match trailing_event {
                        Event::End(_) => (),
                        e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                    }

                    Ok(Some(DefLight {
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
        let mut timestamp: Option<Timestamp> = None;
        let mut message: Option<String> = None;

        for attr in start_event.attributes() {
            let attr = attr?;
            let attr_value = attr.decode_and_unescape_value(xml_reader)?.into_owned();
            match attr.key {
                QName(b"device") => device = Some(attr_value),
                QName(b"name") => name = Some(attr_value),
                QName(b"state") => state = Some(PropertyState::try_from(attr, xml_reader)?),
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
        Ok(SetLightVector {
            device: device.ok_or(DeError::MissingAttr(&"device"))?,
            name: name.ok_or(DeError::MissingAttr(&"name"))?,
            state: state.ok_or(DeError::MissingAttr(&"state"))?,
            timestamp: timestamp,
            message: message,
            lights: Vec::new(),
        })
    }

    fn next_light(&mut self) -> Result<Option<OneLight>, DeError> {
        let event = self.xml_reader.read_event_into(&mut self.buf)?;
        match event {
            Event::Start(e) => match e.name() {
                QName(b"oneLight") => {
                    let mut name: Result<String, DeError> = Err(DeError::MissingAttr(&"name"));

                    for attr in e.attributes() {
                        let attr = attr?;
                        let attr_value = attr
                            .decode_and_unescape_value(self.xml_reader)?
                            .into_owned();

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

                    let value: Result<PropertyState, DeError> =
                        match self.xml_reader.read_event_into(self.buf) {
                            Ok(Event::Text(e)) => PropertyState::try_from_event(e),
                            e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                        };

                    let trailing_event = self.xml_reader.read_event_into(&mut self.buf)?;
                    match trailing_event {
                        Event::End(_) => (),
                        e => return Err(DeError::UnexpectedEvent(format!("{:?}", e))),
                    }

                    Ok(Some(OneLight {
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

        let result: DefLight = quick_xml::de::from_str(xml).unwrap();

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

        let result: OneLight = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(
            result,
            OneLight {
                name: "INDI_DISABLED".to_string(),
                value: PropertyState::Ok
            }
        );
    }

    #[test]
    fn test_def_light_vector() {
        let xml = r#"
    <defLightVector device="CCD Simulator" name="SIMULATE_BAYER" label="Bayer" group="Simulator Config" state="Idle" timestamp="2022-09-06T01:41:22">
    <defLight name="INDI_ENABLED" label="Enabled">
    Busy
    </defLight>
    <defLight name="INDI_DISABLED" label="Disabled">
    Ok
    </defLight>
    </defLightVector>
                    "#;
        let param: DefLightVector = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(param.device, "CCD Simulator");
        assert_eq!(param.name, "SIMULATE_BAYER");
        assert_eq!(param.lights.len(), 2)
    }

    #[test]
    fn test_set_light_vector() {
        let xml = r#"
    <setLightVector device="CCD Simulator" name="SIMULATE_BAYER" state="Idle" timestamp="2022-09-06T01:41:22">
    <oneLight name="INDI_ENABLED">
    Busy
    </oneLight>
    <oneLight name="INDI_DISABLED">
    Ok
    </oneLight>
    </setLightVector>
                    "#;
        let param: SetLightVector = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(param.device, "CCD Simulator");
        assert_eq!(param.name, "SIMULATE_BAYER");
        assert_eq!(param.lights.len(), 2)
    }
}
