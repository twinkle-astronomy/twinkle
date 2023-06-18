use super::*;

#[test]
fn test_def_light_vector() {
    let xml = r#"
<defLightVector device="CCD Simulator" name="SIMULATE_BAYER" label="Bayer" group="Simulator Config" state="Idle" timestamp="2022-09-06T01:41:22">
<defLight name="INDI_ENABLED" label="Enabled">
Busy
</defLight>
<defLight name="INDI_DISABLED" label="Disabled">
Ok
</defLight>
</defLightVector>
                "#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::DefLightVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "SIMULATE_BAYER");
            assert_eq!(param.lights.len(), 2)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_set_light_vector() {
    let xml = r#"
<setLightVector device="CCD Simulator" name="SIMULATE_BAYER" state="Idle" timestamp="2022-09-06T01:41:22">
<oneLight name="INDI_ENABLED">
Busy
</oneLight>
<oneLight name="INDI_DISABLED">
Ok
</oneLight>
</setLightVector>
                "#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::SetLightVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "SIMULATE_BAYER");
            assert_eq!(param.lights.len(), 2)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_blob_vector() {
    let xml = r#"
<defBLOBVector device="CCD Simulator" name="SIMULATE_BAYER" label="Bayer" group="Simulator Config" perm="rw"  state="Idle" timestamp="2022-09-06T01:41:22">
<defBLOB name="INDI_ENABLED" label="Enabled"/>
<defBLOB name="INDI_DISABLED" label="Disabled"/>
</defBLOBVector>
                "#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::DefBlobVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "SIMULATE_BAYER");
            assert_eq!(param.blobs.len(), 2)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_set_blob_vector() {
    let xml = include_str!("../../tests/image_capture_blob_vector.log");

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    reader.expand_empty_elements(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::SetBlobVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "CCD1");
            assert_eq!(param.state, PropertyState::Ok);
            assert_eq!(param.blobs.len(), 1)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

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
