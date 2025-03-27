use super::super::*;
use super::*;

impl CommandtoParam for DefTextVector {
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_group(&self) -> &Option<String> {
        &self.group
    }
    fn to_param(self) -> Parameter {
        Parameter::TextVector(TextVector {
            name: self.name,
            group: self.group,
            label: self.label,
            state: self.state,
            perm: self.perm,
            timeout: self.timeout,
            timestamp: self.timestamp.map(Timestamp::into_inner),
            values: self
                .texts
                .into_iter()
                .map(|i| {
                    (
                        i.name,
                        Text {
                            label: i.label,
                            value: i.value,
                        },
                    )
                })
                .collect(),
        })
    }
}

impl CommandToUpdate for SetTextVector {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn update_param(self, param: &mut Parameter) -> Result<String, UpdateError> {
        match param {
            Parameter::TextVector(text_vector) => {
                text_vector.state = self.state;
                text_vector.timeout = self.timeout;
                text_vector.timestamp = self.timestamp.map(Timestamp::into_inner);
                for text in self.texts {
                    if let Some(existing) = text_vector.values.get_mut(&text.name) {
                        existing.value = text.value;
                    }
                }
                Ok(self.name)
            }
            _ => Err(UpdateError::ParameterTypeMismatch(self.name.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_def_text_vector() {
        let xml = r#"
    <defTextVector device="CCD Simulator" name="ACTIVE_DEVICES" label="Snoop devices" group="Options" state="Idle" perm="rw" timeout="60" timestamp="2022-09-05T21:07:22">
    <defText name="ACTIVE_TELESCOPE" label="Telescope">
    Telescope Simulator
    </defText>
    <defText name="ACTIVE_ROTATOR" label="Rotator">
    Rotator Simulator
    </defText>
    <defText name="ACTIVE_FOCUSER" label="Focuser">
    Focuser Simulator
    </defText>
    <defText name="ACTIVE_FILTER" label="Filter">
    CCD Simulator
    </defText>
    <defText name="ACTIVE_SKYQUALITY" label="Sky Quality">
    SQM
    </defText>
    </defTextVector>
                    "#;
        let command: Result<DefTextVector, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "ACTIVE_DEVICES");
                assert_eq!(param.texts.len(), 5)
            }
            Err(e) => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_def_text() {
        let xml = r#"
    <defText name="ACTIVE_TELESCOPE" label="Active Telescope">
        Simulator changed
    </defText>
    "#;
        let command: Result<DefText, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.name, "ACTIVE_TELESCOPE");
                assert_eq!(param.label, Some(String::from("Active Telescope")));
                assert_eq!(param.value, "Simulator changed")
            }
            Err(e) => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_one_text() {
        let xml = r#"
    <oneText name="ACTIVE_TELESCOPE">
        Simulator changed
    </oneText>
    "#;
        let command: Result<OneText, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.name, "ACTIVE_TELESCOPE");
                assert_eq!(param.value, "Simulator changed")
            }
            Err(e) => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }
    #[test]
    fn test_set_text_vector() {
        let xml = r#"
    <setTextVector device="CCD Simulator" name="ACTIVE_DEVICES" state="Ok" timeout="60" timestamp="2022-10-01T17:06:14">
    <oneText name="ACTIVE_TELESCOPE">
    Simulator changed
    </oneText>
    <oneText name="ACTIVE_ROTATOR">
    Rotator Simulator
    </oneText>
    <oneText name="ACTIVE_FOCUSER">
    Focuser Simulator
    </oneText>
    <oneText name="ACTIVE_FILTER">
    CCD Simulator
    </oneText>
    <oneText name="ACTIVE_SKYQUALITY">
    SQM
    </oneText>
    </setTextVector>
                    "#;
        let command: Result<SetTextVector, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "ACTIVE_DEVICES");
                assert_eq!(param.texts.len(), 5)
            }
            Err(e) => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_new_text_vector() {
        let xml = r#"
    <newTextVector device="CCD Simulator" name="ACTIVE_DEVICES" timestamp="2022-10-01T17:06:14">
    <oneText name="ACTIVE_TELESCOPE">
    Simulator changed
    </oneText>
    <oneText name="ACTIVE_ROTATOR">
    Rotator Simulator
    </oneText>
    <oneText name="ACTIVE_FOCUSER">
    Focuser Simulator
    </oneText>
    <oneText name="ACTIVE_FILTER">
    CCD Simulator
    </oneText>
    <oneText name="ACTIVE_SKYQUALITY">
    SQM
    </oneText>
    </newTextVector>
                    "#;
        let command: Result<NewTextVector, _> = quick_xml::de::from_str(xml);
        match command {
            Ok(param) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "ACTIVE_DEVICES");
                assert_eq!(param.texts.len(), 5)
            }
            Err(e) => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }
}
