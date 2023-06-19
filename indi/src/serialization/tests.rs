use super::*;

#[test]
fn test_message() {
    let xml = r#"
<message device="Telescope Simulator" timestamp="2022-10-02T00:37:07" message="[INFO] update mount and pier side: Pier Side On, mount type 2"/>
                "#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::Message(param) => {
            assert_eq!(param.device, Some(String::from("Telescope Simulator")));
            assert_eq!(
                param.message,
                Some(String::from(
                    "[INFO] update mount and pier side: Pier Side On, mount type 2"
                ))
            );
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_get_properties() {
    let xml = r#"
<getProperties version="1.7" device="Telescope Simulator" name="foothing"/>
                "#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::GetProperties(param) => {
            assert_eq!(param.device, Some(String::from("Telescope Simulator")));
            assert_eq!(param.name, Some(String::from("foothing")));
            assert_eq!(param.version, String::from("1.7"));
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_set_simulator_log() {
    let xml = include_str!("../../tests/image_capture.log");

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);
    let mut command_iter = CommandIter::new(reader);

    for command in command_iter.by_ref() {
        match command {
            Ok(_) => (),
            Err(e) => {
                println!("position: {}", command_iter.buffer_position());
                panic!("{:?}", e);
            }
        }
    }
}
