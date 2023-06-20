use std::io::Cursor;

use super::*;

#[test]
fn test_set_simulator_log() {
    let xml = include_str!("../../tests/image_capture.log");

    let mut command_iter = CommandIter::new(Cursor::new(xml));

    for command in command_iter.by_ref() {
        match command {
            Ok(c) => {
                dbg!(c);
            }
            Err(e) => {
                panic!("{:?}", e);
            }
        }
    }
}
