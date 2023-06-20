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
