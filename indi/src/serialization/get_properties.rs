use quick_xml::Writer;

use super::super::*;
use super::*;

impl XmlSerialization for GetProperties {
    fn write<'a, T: std::io::Write>(
        &self,
        xml_writer: &'a mut Writer<T>,
    ) -> XmlResult<&'a mut Writer<T>> {
        let mut creator = xml_writer
            .create_element("getProperties")
            .with_attribute(("version", &*self.version));

        if let Some(device) = &self.device {
            creator = creator.with_attribute(("device", &device[..]));
        }
        if let Some(name) = &self.name {
            creator = creator.with_attribute(("name", &name[..]));
        }

        creator.write_empty()?;
        Ok(xml_writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_send_get_properties() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        let command = GetProperties {
            version: String::from_str("1.7").unwrap(),
            device: Some(String::from_str("CCD Simulator").unwrap()),
            name: None,
        };

        command.write(&mut writer).unwrap();

        let result = writer.into_inner().into_inner();
        assert_eq!(
            String::from_utf8(result).unwrap(),
            String::from_str("<getProperties version=\"1.7\" device=\"CCD Simulator\"/>").unwrap()
        );
    }

    #[test]
    fn test_get_properties() {
        let xml = r#"
    <getProperties version="1.7" device="Telescope Simulator" name="foothing"/>
                    "#;
        let param: GetProperties = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(param.device, Some(String::from("Telescope Simulator")));
        assert_eq!(param.name, Some(String::from("foothing")));
    }
}
