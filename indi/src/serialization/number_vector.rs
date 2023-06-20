use std::str;

// use encoding::all::ISO_8859_1;
// use encoding::{DecoderTrap, Encoding};

use super::super::*;
use super::*;

impl<'de> Deserialize<'de> for Sexagesimal {
    fn deserialize<D>(deserializer: D) -> Result<Sexagesimal, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let mut components = s.split([' ', ':']);

        let hour = components
            .next()
            .map(str::parse)
            .transpose()
            .unwrap()
            .unwrap();
        let minute = components.next().map(str::parse).transpose().unwrap();
        let second = components.next().map(str::parse).transpose().unwrap();

        Ok(Sexagesimal {
            hour,
            minute,
            second,
        })
    }
}

impl std::fmt::Display for Sexagesimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hour)?;
        if let Some(minute) = self.minute {
            write!(f, ":{}", minute)?;
        }
        if let Some(second) = self.second {
            write!(f, ":{}", second)?;
        }

        Ok(())
    }
}

impl From<f64> for Sexagesimal {
    fn from(value: f64) -> Self {
        // TODO: try splitting minute and second out of value instead of putting
        //  it all in hour.
        Self {
            hour: value.into(),
            minute: None,
            second: None,
        }
    }
}

impl From<Sexagesimal> for f64 {
    fn from(value: Sexagesimal) -> Self {
        let mut val = value.hour;

        let sign = value.hour.signum();
        let div = 60.0;

        if let Some(minute) = value.minute {
            val += sign * minute / div;
        }
        if let Some(second) = value.second {
            val += sign * second / (div * div);
        }

        val
    }
}

impl CommandtoParam for DefNumberVector {
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_group(&self) -> &Option<String> {
        &self.group
    }
    fn to_param(self, gen: Wrapping<usize>) -> Parameter {
        Parameter::NumberVector(NumberVector {
            gen,
            name: self.name,
            group: self.group,
            label: self.label,
            state: self.state,
            perm: self.perm,
            timeout: self.timeout,
            timestamp: self.timestamp.map(Timestamp::into_inner),
            values: self
                .numbers
                .into_iter()
                .map(|i| {
                    (
                        i.name,
                        Number {
                            label: i.label,
                            format: i.format,
                            min: i.min,
                            max: i.max,
                            step: i.step,
                            value: i.value,
                        },
                    )
                })
                .collect(),
        })
    }
}

impl CommandToUpdate for SetNumberVector {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn update_param(self, param: &mut Parameter) -> Result<String, UpdateError> {
        match param {
            Parameter::NumberVector(number_vector) => {
                number_vector.state = self.state;
                number_vector.timeout = self.timeout;
                number_vector.timestamp = self.timestamp.map(Timestamp::into_inner);
                for number in self.numbers {
                    if let Some(existing) = number_vector.values.get_mut(&number.name) {
                        existing.min = number.min.unwrap_or(existing.min);
                        existing.max = number.max.unwrap_or(existing.max);
                        existing.step = number.step.unwrap_or(existing.step);
                        existing.value = number.value;
                    }
                }
                Ok(self.name)
            }
            _ => Err(UpdateError::ParameterTypeMismatch(self.name.clone())),
        }
    }
}

impl XmlSerialization for SetOneNumber {
    fn write<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let mut creator = xml_writer
            .create_element("oneNumber")
            .with_attribute(("name", &*self.name));

        if let Some(min) = &self.min {
            creator = creator.with_attribute(("min", min.to_string().as_str()));
        }
        if let Some(max) = &self.max {
            creator = creator.with_attribute(("max", max.to_string().as_str()));
        }
        if let Some(step) = &self.step {
            creator = creator.with_attribute(("step", step.to_string().as_str()));
        }
        creator.write_text_content(BytesText::new(self.value.to_string().as_str()))?;

        Ok(xml_writer)
    }
}

impl XmlSerialization for OneNumber {
    fn write<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let creator = xml_writer
            .create_element("oneNumber")
            .with_attribute(("name", &*self.name));

        creator.write_text_content(BytesText::new(self.value.to_string().as_str()))?;

        Ok(xml_writer)
    }
}

impl XmlSerialization for NewNumberVector {
    fn write<'a, T: std::io::Write>(
        &self,
        mut xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        {
            let mut creator = xml_writer
                .create_element("newNumberVector")
                .with_attribute(("device", &*self.device))
                .with_attribute(("name", &*self.name));

            if let Some(timestamp) = &self.timestamp {
                creator = creator.with_attribute((
                    "timestamp",
                    format!("{}", timestamp.deref().format("%Y-%m-%dT%H:%M:%S%.3f")).as_str(),
                ));
            }
            xml_writer = creator.write_inner_content(|xml_writer| {
                for number in self.numbers.iter() {
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
    fn test_def_number() {
        let xml = r#"
        <defNumber name="SIM_XRES" label="CCD X resolution" format="%4.0f" min="512" max="8192" step="512">
    1280
        </defNumber>
                    "#;
        let command: Result<DefNumber, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.name, "SIM_XRES");
                assert_eq!(param.label, Some(String::from("CCD X resolution")));
                assert_eq!(param.value, 1280.0.into());
            }
            Err(e) => {
                panic!("Unexpected: {:?}", e);
            }
        }
    }

    #[test]
    fn test_def_number_vector() {
        let xml = r#"
    <defNumberVector device="CCD Simulator" name="SIMULATOR_SETTINGS" label="Settings" group="Simulator Config" state="Idle" perm="rw" timeout="60" timestamp="2022-08-12T05:52:27">
        <defNumber name="SIM_XRES" label="CCD X resolution" format="%4.0f" min="512" max="8192" step="512">
    1280
        </defNumber>
        <defNumber name="SIM_YRES" label="CCD Y resolution" format="%4.0f" min="512" max="8192" step="512">
    1024
        </defNumber>
        <defNumber name="SIM_XSIZE" label="CCD X Pixel Size" format="%4.2f" min="1" max="30" step="5">
    5.2000000000000001776
        </defNumber>
    </defNumberVector>
                    "#;
        let command: Result<DefNumberVector, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "SIMULATOR_SETTINGS");
                assert_eq!(param.numbers.len(), 3)
            }
            e => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_set_number_vector() {
        let xml = r#"
    <setNumberVector device="CCD Simulator" name="SIM_FOCUSING" state="Ok" timeout="60" timestamp="2022-10-01T21:21:10">
    <oneNumber name="SIM_FOCUS_POSITION">
    7340
    </oneNumber>
    <oneNumber name="SIM_FOCUS_MAX">
    100000
    </oneNumber>
    <oneNumber name="SIM_SEEING">
    3.5
    </oneNumber>
    </setNumberVector>
"#;

        let command: Result<SetNumberVector, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "SIM_FOCUSING");
                assert_eq!(param.numbers.len(), 3)
            }
            e => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_new_number_vector() {
        let xml = r#"
    <newNumberVector device="CCD Simulator" name="SIM_FOCUSING" timestamp="2022-10-01T21:21:10">
    <oneNumber name="SIM_FOCUS_POSITION">
    7340
    </oneNumber>
    <oneNumber name="SIM_FOCUS_MAX">
    100000
    </oneNumber>
    <oneNumber name="SIM_SEEING">
    3.5
    </oneNumber>
    </newNumberVector>
    "#;

        let command: Result<NewNumberVector, _> = quick_xml::de::from_str(xml);

        match command {
            Ok(param) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "SIM_FOCUSING");
                assert_eq!(param.numbers.len(), 3)
            }
            e => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_parse_number_normal() {
        let xml = r#"-10.505"#;

        let event: Result<Sexagesimal, _> = quick_xml::de::from_str(xml);

        if let Ok(e) = event {
            assert_eq!(Into::<Sexagesimal>::into(-10.505), e);
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_parse_number_sexagesimal_1() {
        let xml = r#"-10 30.3"#;

        let event: Result<Sexagesimal, _> = quick_xml::de::from_str(xml);

        if let Ok(e) = event {
            assert_eq!(-10.505, e.into());
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_parse_number_sexagesimal_2() {
        let xml = r#"-10:30:18"#;

        let event: Result<Sexagesimal, _> = quick_xml::de::from_str(xml);

        if let Ok(e) = event {
            assert_eq!(-10.505, e.into());
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_send_new_number_vector() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z")
            .unwrap()
            .into();

        let command = NewNumberVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            timestamp: Some(timestamp),
            numbers: vec![OneNumber {
                name: String::from_str("seconds").unwrap(),
                value: 3.0.into(),
            }],
        };

        command.write(&mut writer).unwrap();

        let result = writer.into_inner().into_inner();
        assert_eq!(
            String::from_utf8(result).unwrap(),
            String::from_str("<newNumberVector device=\"CCD Simulator\" name=\"Exposure\" timestamp=\"2022-10-13T07:41:56.301\"><oneNumber name=\"seconds\">3</oneNumber></newNumberVector>").unwrap()
        );
    }
}
