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
            timestamp: self.timestamp.map(Timestamp::into_inner),
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
                switch_vector.timestamp = self.timestamp.map(Timestamp::into_inner);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_switch() {
        let xml = r#"<defSwitch name="INDI_DISABLED" label="Disabled">On</defSwitch>"#;
        let command: DefSwitch = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(
            command,
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

        let command: DefSwitch = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(
            command,
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

        let command: OneSwitch = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(
            command,
            OneSwitch {
                name: "INDI_DISABLED".to_string(),
                value: SwitchState::On
            }
        );
    }

    #[test]
    fn test_send_new_switch_vector() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z")
            .unwrap()
            .into();

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

    #[test]
    fn test_def_switch_vector() {
        let xml = r#"
    <defSwitchVector device="CCD Simulator" name="SIMULATE_BAYER" label="Bayer" group="Simulator Config" state="Idle" perm="rw" rule="OneOfMany" timeout="60" timestamp="2022-09-06T01:41:22">
    <defSwitch name="INDI_ENABLED" label="Enabled">
    Off
    </defSwitch>
    <defSwitch name="INDI_DISABLED" label="Disabled">
    On
    </defSwitch>
    </defSwitchVector>
                    "#;
        let param: DefSwitchVector = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(param.device, "CCD Simulator");
        assert_eq!(param.name, "SIMULATE_BAYER");
        assert_eq!(param.switches.len(), 2)
    }

    #[test]
    fn test_set_switch_vector() {
        let xml = r#"
    <setSwitchVector device="CCD Simulator" name="DEBUG" state="Ok" timeout="0" timestamp="2022-10-01T22:07:22">
    <oneSwitch name="ENABLE">
    On
    </oneSwitch>
    <oneSwitch name="DISABLE">
    Off
    </oneSwitch>
    </setSwitchVector>
                    "#;
        let param: SetSwitchVector = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(param.device, "CCD Simulator");
        assert_eq!(param.name, "DEBUG");
        assert_eq!(param.switches.len(), 2)
    }

    #[test]
    fn test_new_switch_vector() {
        let xml = r#"
    <newSwitchVector device="CCD Simulator" name="DEBUG" timestamp="2022-10-01T22:07:22">
    <oneSwitch name="ENABLE">
    On
    </oneSwitch>
    <oneSwitch name="DISABLE">
    Off
    </oneSwitch>
    </newSwitchVector>
                    "#;
        let param: NewSwitchVector = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(param.device, "CCD Simulator");
        assert_eq!(param.name, "DEBUG");
        assert_eq!(param.switches.len(), 2)
    }
}
