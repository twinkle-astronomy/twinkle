#[cfg(test)]
mod tests {
    use crate::serialization::Message;

    #[test]
    fn test_message() {
        let xml = r#"
    <message device="Telescope Simulator" timestamp="2022-10-02T00:37:07" message="[INFO] update mount and pier side: Pier Side On, mount type 2"/>
                    "#;
        let param: Message = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(param.device, Some(String::from("Telescope Simulator")));
        assert_eq!(
            param.message,
            Some(String::from(
                "[INFO] update mount and pier side: Pier Side On, mount type 2"
            ))
        )
    }
}
