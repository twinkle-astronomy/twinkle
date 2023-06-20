#[cfg(test)]
mod tests {
    use crate::serialization::DelProperty;

    #[test]
    fn test_get_properties() {
        let xml = r#"
    <delProperty device="Telescope Simulator" name="foothing"/>
                    "#;
        let param: DelProperty = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(param.device, String::from("Telescope Simulator"));
        assert_eq!(param.name, Some(String::from("foothing")));
    }
}
