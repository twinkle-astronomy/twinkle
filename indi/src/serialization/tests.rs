use super::*;

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
