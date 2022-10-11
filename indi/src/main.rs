use indi;

fn main() {
    let mut client = indi::Client::new("localhost:7624").unwrap();
    client.send(&indi::GetProperties {
        version: indi::INDI_PROTOCOL_VERSION.to_string(),
        device: None,
        name: None,
    }).unwrap();

    for command in client.command_iter().unwrap() {
        match command {
            Ok(command) => {
                println!("entry: {:?}", command);
            }
            Err(e) => match e {
                e => println!("error: {:?}", e),
            },
        }
    }
}

/*
<getProperties/>
<enableBLOB device="CCD Simulator">Also</enableBLOB>
<newSwitchVector  device="CCD Simulator" name="CONNECTION">
    <oneSwitch name="CONNECT">
On
    </oneSwitch>
</newSwitchVector>

<newSwitchVector  device="Telescope Simulator" name="CONNECTION">
    <oneSwitch name="CONNECT">
On
    </oneSwitch>
</newSwitchVector>

<newNumberVector device="CCD Simulator" name="CCD_FRAME" state="Ok" timeout="60" timestamp="2022-10-02T00:36:02">
    <oneNumber name="X">
0
    </oneNumber>
    <oneNumber name="Y">
0
    </oneNumber>
    <oneNumber name="WIDTH">
10
    </oneNumber>
    <oneNumber name="HEIGHT">
10
    </oneNumber>
</newNumberVector>

<newNumberVector device="CCD Simulator" name="CCD_EXPOSURE" label="Expose" group="Main Control" state="Idle" perm="rw" timeout="60" timestamp="2022-10-01T23:34:57">
    <oneNumber name="CCD_EXPOSURE_VALUE">
1
    </oneNumber>
</newNumberVector>

*/
