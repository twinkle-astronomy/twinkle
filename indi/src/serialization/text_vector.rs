use super::super::*;
use super::*;

impl CommandtoParam for DefTextVector {
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_group(&self) -> &Option<String> {
        &self.group
    }
    fn to_param(self, gen: Wrapping<usize>) -> Parameter {
        Parameter::TextVector(TextVector {
            gen,
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

impl XmlSerialization for OneText {
    fn write<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let creator = xml_writer
            .create_element("oneText")
            .with_attribute(("name", &*self.name));

        creator.write_text_content(BytesText::new(self.value.as_str()))?;

        Ok(xml_writer)
    }
}

impl XmlSerialization for NewTextVector {
    fn write<'a, T: std::io::Write>(
        &self,
        mut xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        {
            let mut creator = xml_writer
                .create_element("newTextVector")
                .with_attribute(("device", &*self.device))
                .with_attribute(("name", &*self.name));

            if let Some(timestamp) = &self.timestamp {
                creator = creator.with_attribute((
                    "timestamp",
                    format!("{}", timestamp.into_inner().format("%Y-%m-%dT%H:%M:%S%.3f")).as_str(),
                ));
            }
            xml_writer = creator.write_inner_content(|xml_writer| {
                for text in self.texts.iter() {
                    text.write(xml_writer)?;
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

    //     use std::io::Cursor;
    //     #[test]
    //     fn test_one_text() {
    //         let xml = b"\
    //     <oneText name=\"ACTIVE_TELESCOPE\">
    // Simulator \xFF changed
    //     </oneText>
    // ";
    //         // Attempt to force quick_xml to use ISO-8859-1 encoding by injecting an encoding specifier at the beginning
    //         //  of the xml stream.
    //         // let preamble_read = Cursor::new(b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\" standalone=\"yes\"><xml/>");
    //         let buf_read = Cursor::new(xml);

    //         // // To use the preamble swap the commenting on the next two lines.
    //         // let mut reader = Reader::from_reader(std::io::Read::chain(preamble_read, buf_read));
    //         let mut reader = Reader::from_reader(buf_read);

    //         reader.trim_text(true);
    //         reader.expand_empty_elements(true);
    //         let mut command_iter = CommandIter::new(reader);
    //         let mut number_iter = SetTextIter::new(&mut command_iter);

    //         let result = number_iter.next().unwrap().unwrap();

    //         assert_eq!(
    //             result,
    //             OneText {
    //                 name: "ACTIVE_TELESCOPE".to_string(),
    //                 value: "Simulator ÿ changed".to_string()
    //             }
    //         );
    //     }

    //     #[test]
    //     fn test_send_new_text_vector() {
    //         let mut writer = Writer::new(Cursor::new(Vec::new()));
    //         let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

    //         let command = NewTextVector {
    //             device: String::from_str("CCD Simulator").unwrap(),
    //             name: String::from_str("Exposure").unwrap(),
    //             timestamp: Some(timestamp),
    //             texts: vec![OneText {
    //                 name: String::from_str("seconds").unwrap(),
    //                 value: String::from_str("Long ÿ enough").unwrap(),
    //             }],
    //         };

    //         command.send(&mut writer).unwrap();

    //         let result = writer.into_inner().into_inner();
    //         assert_eq!(
    //             result,
    //             b"<newTextVector device=\"CCD Simulator\" name=\"Exposure\" timestamp=\"2022-10-13T07:41:56.301\"><oneText name=\"seconds\">Long \xFF enough</oneText></newTextVector>"
    //         );
    //     }
}
