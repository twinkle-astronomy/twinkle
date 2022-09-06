pub mod number_vector;

pub use number_vector::NumberIter;

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
    fn test_listen_for_updates() {
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

        match command_iter.next().unwrap() {
            Ok(Command::DefParameter(Parameter::Number(nv))) => {
                assert_eq!(nv.device, "CCD Simulator");
                assert_eq!(nv.numbers.len(), 3)
            }
            _ => {panic!("Unexpected next")}
        }

    }


//     #[test]
//     fn test_parse_numbervector() {
//         let mut buf = Vec::new();
//         let xml = r#"
// <defNumberVector device="CCD Simulator" name="SIMULATOR_SETTINGS" label="Settings" group="Simulator Config" state="Idle" perm="rw" timeout="60" timestamp="2022-08-12T05:52:27">
//     <defNumber name="SIM_XRES" label="CCD X resolution" format="%4.0f" min="512" max="8192" step="512">
// 1280
//     </defNumber>
//     <defNumber name="SIM_YRES" label="CCD Y resolution" format="%4.0f" min="512" max="8192" step="512">
// 1024
//     </defNumber>
//     <defNumber name="SIM_XSIZE" label="CCD X Pixel Size" format="%4.2f" min="1" max="30" step="5">
// 5.2000000000000001776
//     </defNumber>
// </defNumberVector>
// "#;

//         let mut reader = Reader::from_str(xml);
//         reader.trim_text(true);
//         let result = match reader.read_event(&mut buf).unwrap() {
//             Event::Start(start_event) => NumberVector::parse(&mut reader, start_event).unwrap(),
//             _ => panic!("wrong element type"),
//         };
//         // let result = Number::parse(reader).unwrap();
//         assert_eq!(result.name, "SIMULATOR_SETTINGS".to_string());
//         assert_eq!(result.device, "CCD Simulator".to_string());
//         assert_eq!(result.label, "Settings".to_string());
//         assert_eq!(result.group, "Simulator Config".to_string());
//         assert_eq!(result.state, "Idle".to_string());
//         assert_eq!(result.perm, "rw".to_string());
//         assert_eq!(result.timeout, 60);
//         assert_eq!(
//             result.timestamp,
//             DateTime::<Utc>::from_str("2022-08-12T05:52:27Z").unwrap()
//         );
//     }
}
