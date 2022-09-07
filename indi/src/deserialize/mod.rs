pub mod number_vector;
pub use number_vector::NumberIter;

pub mod text_vector;
pub use text_vector::TextIter;

pub mod switch_vector;
pub use switch_vector::SwitchIter;

use super::*;

pub struct CommandIter<T: std::io::BufRead> {
    xml_reader: Reader<T>,
    buf: Vec<u8>,
}


impl<T: std::io::BufRead> Iterator for CommandIter<T> {
    type Item = Result<Command, DeError>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_command() {
            Ok(Some(command)) => {
                return Some(Ok(command));
            }
            Ok(None) => return None,
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
}

impl<T: std::io::BufRead> CommandIter<T> {
    pub fn new(xml_reader: Reader<T>) -> CommandIter<T> {
        let buf = Vec::new();
        CommandIter { xml_reader, buf }
    }

    fn next_command(&mut self) -> Result<Option<Command>, DeError> {
        let event = self.xml_reader.read_event(&mut self.buf)?;
        match event {
            Event::Start(e) => {
                let result = match e.name() {
                    b"defTextVector" => {
                        let mut text_vector = TextIter::def_text_vector(&self.xml_reader, &e)?;

                        for text in deserialize::TextIter::new(self) {
                            let text = text?;
                            text_vector.texts.insert(text.name.clone(), text);
                        }

                        Ok(Some(Command::DefParameter(Parameter::Text(text_vector))))
                    },
                    b"defNumberVector" => {
                        let mut number_vector =
                            NumberIter::def_number_vector(&self.xml_reader, &e)?;

                        for number in deserialize::NumberIter::new(self) {
                            let number = number?;
                            number_vector.numbers.insert(number.name.clone(), number);
                        }

                        Ok(Some(Command::DefParameter(Parameter::Number(
                            number_vector,
                        ))))
                    },
                    b"defSwitchVector" => {
                        let mut switch_vector = SwitchIter::def_switch_vector(&self.xml_reader, &e)?;

                        for switch in deserialize::SwitchIter::new(self) {
                            let switch = switch?;
                            switch_vector.switches.insert(switch.name.clone(), switch);
                        }

                        Ok(Some(Command::DefParameter(Parameter::Switch(switch_vector))))
                    }
                    tag => Err(DeError::UnexpectedTag(str::from_utf8(tag)?.to_string())),
                };
                result
            }
            Event::End(tag) => {
                println!("Unexpected end: {}", tag.escape_ascii().to_string());
                Err(DeError::UnexpectedEvent())
            }
            Event::Eof => Ok(None),
            _ => Err(DeError::UnexpectedEvent()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_number_vector() {
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
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);

        match command_iter.next().unwrap().unwrap() {
            Command::DefParameter(Parameter::Number(param)) => {
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
    fn test_text_vector() {
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
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);

        match command_iter.next().unwrap().unwrap() {
            Command::DefParameter(Parameter::Text(param)) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "ACTIVE_DEVICES");
                assert_eq!(param.texts.len(), 5)
            }
            e => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }

    #[test]
    fn test_switch_vector() {
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
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut command_iter = CommandIter::new(reader);

        match command_iter.next().unwrap().unwrap() {
            Command::DefParameter(Parameter::Switch(param)) => {
                assert_eq!(param.device, "CCD Simulator");
                assert_eq!(param.name, "SIMULATE_BAYER");
                assert_eq!(param.switches.len(), 2)
            }
            e => {
                panic!("Unexpected: {:?}", e)
            }
        }
    }
}
