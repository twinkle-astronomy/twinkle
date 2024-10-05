
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::GetProperties;

    #[test]
    fn test_send_get_properties() {
        // let mut writer = Writer::new(Cursor::new(Vec::new()));

        let command = GetProperties {
            version: String::from_str("1.7").unwrap(),
            device: Some(String::from_str("CCD Simulator").unwrap()),
            name: None,
        };

        let result = quick_xml::se::to_string(&command).unwrap();
        assert_eq!(
            result,
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
