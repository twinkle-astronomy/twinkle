use std::env;
use std::fs::File;
use std::io::prelude::*;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut connection = indi::Connection::new(&args[1]).unwrap();
    connection
        .write(&indi::GetProperties {
            version: indi::INDI_PROTOCOL_VERSION.to_string(),
            device: None,
            name: None,
        })
        .expect("Unable to write command");

    connection
        .write(&indi::EnableBlob {
            device: String::from("ZWO CCD ASI294MM Pro"),
            name: None,
            enabled: indi::BlobEnable::Also,
        })
        .expect("Unable to write command");
    
    let mut image_number = 0;
    let mut client = indi::Client::new();

    for command in connection.iter().unwrap() {
        
        match command {
            Ok(indi::Command::SetBlobVector(mut sbv)) => {
                println!("Got image for: {:?}", sbv.device);
                let focus_position = if let indi::Parameter::NumberVector(focus_position) = &client.get_devices()["ASI EAF"].get_parameters()["ABS_FOCUS_POSITION"]{
                    focus_position.values["FOCUS_ABSOLUTE_POSITION"].value
                } else {0.0};
                image_number += 1;
                let filename = format!("{} {} {:02}.fits", sbv.device, focus_position, image_number);
                File::create(filename)
                    .expect("Unable to create file")
                    .write_all(&mut sbv.blobs.get_mut(0).unwrap().value)
                    .expect("Unable to write file");
            }
            _ => {
                client.update(command.unwrap());
            }
        }
    }
}