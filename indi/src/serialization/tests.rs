use super::*;

#[test]
fn test_def_number_vector() {
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

    match command_iter.next().unwrap().unwrap() {
        Command::DefNumberVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "SIMULATOR_SETTINGS");
            assert_eq!(param.numbers.len(), 3)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_set_number_vector() {
    let xml = r#"
<setNumberVector device="CCD Simulator" name="SIM_FOCUSING" state="Ok" timeout="60" timestamp="2022-10-01T21:21:10">
<oneNumber name="SIM_FOCUS_POSITION">
7340
</oneNumber>
<oneNumber name="SIM_FOCUS_MAX">
100000
</oneNumber>
<oneNumber name="SIM_SEEING">
3.5
</oneNumber>
</setNumberVector>
"#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::SetNumberVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "SIM_FOCUSING");
            assert_eq!(param.numbers.len(), 3)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_new_number_vector() {
    let xml = r#"
<newNumberVector device="CCD Simulator" name="SIM_FOCUSING" timestamp="2022-10-01T21:21:10">
<oneNumber name="SIM_FOCUS_POSITION">
7340
</oneNumber>
<oneNumber name="SIM_FOCUS_MAX">
100000
</oneNumber>
<oneNumber name="SIM_SEEING">
3.5
</oneNumber>
</newNumberVector>
"#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::NewNumberVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "SIM_FOCUSING");
            assert_eq!(param.numbers.len(), 3)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

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
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::DefTextVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "ACTIVE_DEVICES");
            assert_eq!(param.texts.len(), 5)
        }
        e => {
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
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::SetTextVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "ACTIVE_DEVICES");
            assert_eq!(param.texts.len(), 5)
        }
        e => {
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
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::NewTextVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "ACTIVE_DEVICES");
            assert_eq!(param.texts.len(), 5)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_def_switch_vector() {
    let xml = r#"
<defSwitchVector device="CCD Simulator" name="SIMULATE_BAYER" label="Bayer" group="Simulator Config" state="Idle" perm="rw" rule="OneOfMany" timeout="60" timestamp="2022-09-06T01:41:22">
<defSwitch name="INDI_ENABLED" label="Enabled">
Off
</defSwitch>
<defSwitch name="INDI_DISABLED" label="Disabled">
On
</defSwitch>
</defSwitchVector>
                "#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::DefSwitchVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "SIMULATE_BAYER");
            assert_eq!(param.switches.len(), 2)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_set_switch_vector() {
    let xml = r#"
<setSwitchVector device="CCD Simulator" name="DEBUG" state="Ok" timeout="0" timestamp="2022-10-01T22:07:22">
<oneSwitch name="ENABLE">
On
</oneSwitch>
<oneSwitch name="DISABLE">
Off
</oneSwitch>
</setSwitchVector>
                "#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::SetSwitchVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "DEBUG");
            assert_eq!(param.switches.len(), 2)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

#[test]
fn test_new_switch_vector() {
    let xml = r#"
<newSwitchVector device="CCD Simulator" name="DEBUG" timestamp="2022-10-01T22:07:22">
<oneSwitch name="ENABLE">
On
</oneSwitch>
<oneSwitch name="DISABLE">
Off
</oneSwitch>
</newSwitchVector>
                "#;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut command_iter = CommandIter::new(reader);

    match command_iter.next().unwrap().unwrap() {
        Command::NewSwitchVector(param) => {
            assert_eq!(param.device, "CCD Simulator");
            assert_eq!(param.name, "DEBUG");
            assert_eq!(param.switches.len(), 2)
        }
        e => {
            panic!("Unexpected: {:?}", e)
        }
    }
}

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
