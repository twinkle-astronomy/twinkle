use std::fmt;
use std::str;

use super::super::*;
use super::*;

impl Sexagesimal {
    fn format_indi_style(&self, precision: usize) -> String {
        let is_negative = self.hour < 0.0;
        let hour = self.hour.abs();

        // Get minute value, defaulting to fractional part of hour if None
        let minute = self.minute.unwrap_or_else(|| (hour - hour.trunc()) * 60.0);

        // Get second value, defaulting to fractional part of minute if None
        let second = self
            .second
            .unwrap_or_else(|| (minute - minute.trunc()) * 60.0);

        // Format based on precision
        let result = match precision {
            9 => format!(
                "{:02}:{:02}.{:02}",
                minute.trunc() as i64,
                second.trunc() as i64,
                ((second % 1.0) * 100.0).round() as i64
            ),
            8 => format!(
                "{:02}:{:02}.{:01}",
                minute.trunc() as i64,
                second.trunc() as i64,
                ((second % 1.0) * 10.0).round() as i64
            ),
            6 => format!("{:02}:{:02}", minute.trunc() as i64, second.round() as i64),
            5 => format!(
                "{:02}.{:01}",
                minute.trunc() as i64,
                ((minute % 1.0) * 10.0).round() as i64
            ),
            3 => format!("{:02}", minute.round() as i64),
            _ => format!("{:02}:{:02}", minute.trunc() as i64, second.round() as i64),
        };

        // Add the hours and sign
        let mut final_result = String::new();
        if is_negative {
            final_result.push('-');
        }
        if hour >= 1.0 || hour.trunc() != 0.0 {
            final_result.push_str(&format!("{}:", hour.trunc() as i64));
        }
        final_result.push_str(&result);

        final_result
    }

    fn format_double(&self) -> f64 {
        let mut value = self.hour;
        if let Some(min) = self.minute {
            value += min / 60.0;
            if let Some(sec) = self.second {
                value += sec / 3600.0;
            }
        }
        value
    }
}

fn parse_m_format(format: &str) -> Option<usize> {
    // Look for pattern %.{n}m where n is the precision
    let parts: Vec<&str> = format.split('.').collect();
    if parts.len() != 2 {
        return None;
    }

    if !parts[1].ends_with('m') {
        return None;
    }

    // Extract the precision number
    let precision_str = &parts[1][..parts[1].len() - 1];
    precision_str.parse().ok()
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Check if this is an m-format
        if let Some(m_precision) = parse_m_format(&self.format) {
            // INDI sexagesimal format
            let formatted = self.value.format_indi_style(m_precision);
            // Handle width alignment if specified
            if let Some(width) = f.width() {
                if f.sign_aware_zero_pad() {
                    write!(f, "{:0>width$}", formatted, width = width)
                } else if f.align() == Some(fmt::Alignment::Left) {
                    write!(f, "{:<width$}", formatted, width = width)
                } else {
                    write!(f, "{:>width$}", formatted, width = width)
                }
            } else {
                write!(f, "{}", formatted)
            }
        } else {
            // Regular double format
            write!(f, "{}", self.value.format_double())
        }
    }
}

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

impl Serialize for Sexagesimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(format!("{}", self).as_str())
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
        Self {
            hour: value as f64,
            minute: None,
            second: None,
        }
    }
}

impl From<usize> for Sexagesimal {
    fn from(value: usize) -> Self {
        Self {
            hour: value as f64,
            minute: None,
            second: None,
        }
    }
}

impl From<u64> for Sexagesimal {
    fn from(value: u64) -> Self {
        Self {
            hour: value as f64,
            minute: None,
            second: None,
        }
    }
}

impl From<i32> for Sexagesimal {
    fn from(value: i32) -> Self {
        Self {
            hour: value as f64,
            minute: None,
            second: None,
        }
    }
}

impl From<u32> for Sexagesimal {
    fn from(value: u32) -> Self {
        Self {
            hour: value as f64,
            minute: None,
            second: None,
        }
    }
}

impl From<u16> for Sexagesimal {
    fn from(value: u16) -> Self {
        Self {
            hour: value as f64,
            minute: None,
            second: None,
        }
    }
}

impl From<u8> for Sexagesimal {
    fn from(value: u8) -> Self {
        Self {
            hour: value as f64,
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
    fn to_param(self) -> Parameter {
        Parameter::NumberVector(NumberVector {
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

#[cfg(test)]
mod tests {
    use super::*;
    // use std::io::Cursor;

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
            assert_eq!(
                Sexagesimal {
                    hour: -10.,
                    minute: Some(30.3),
                    second: None
                },
                e.into()
            );
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_parse_number_sexagesimal_2() {
        let xml = r#"-10:30:18"#;

        let event: Result<Sexagesimal, _> = quick_xml::de::from_str(xml);

        if let Ok(e) = event {
            assert_eq!(
                Sexagesimal {
                    hour: -10.0,
                    minute: Some(30.),
                    second: Some(18.)
                },
                e.into()
            );
        } else {
            panic!("Unexpected");
        }
    }

    #[test]
    fn test_send_new_number_vector() {
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

        // command.write(&mut writer).unwrap();

        // let result = writer.into_inner().into_inner();
        let result = quick_xml::se::to_string(&command).unwrap();
        assert_eq!(
            result,
            String::from_str("<newNumberVector device=\"CCD Simulator\" name=\"Exposure\" timestamp=\"2022-10-13T07:41:56.301\"><oneNumber name=\"seconds\">3</oneNumber></newNumberVector>").unwrap()
        );
    }
}
