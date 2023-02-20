use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::net::TcpStream;
use std::time::Duration;

use indi::client::notify;

fn main() {
    let args: Vec<String> = env::args().collect();

    let client = indi::Client::new(TcpStream::connect(&args[1]).unwrap(), None, None)
        .expect("connecting to indi server");
    let camera = client.get_device("ZWO CCD ASI294MM Pro").unwrap();
    camera
        .enable_blob(None, indi::BlobEnable::Also)
        .expect("enabling image delivery");
    let imager = camera.get_parameter("CCD1").unwrap();

    let eaf = client.get_device("ASI EAF").unwrap();

    let mut image_number = 0;
    let mut imager_gen = imager.lock().gen();

    imager
        .wait_fn::<(), (), _>(Duration::MAX, |imager| {
            if imager.gen() == imager_gen {
                return Ok(notify::Status::Pending);
            }
            imager_gen = imager.gen();

            let ccd = imager
                .get_values::<HashMap<String, indi::Blob>>()?
                .get("CCD1")
                .unwrap();
            if let Some(image) = &ccd.value {
                let afp = eaf
                    .get_parameter("ABS_FOCUS_POSITION")
                    .expect("getting focus position");
                let abs = afp.lock();
                let focus_position = abs
                    .get_values::<HashMap<String, indi::Number>>()?
                    .get("FOCUS_ABSOLUTE_POSITION")
                    .unwrap()
                    .value;

                image_number += 1;
                let filename = format!(
                    "{} {} {:02}.fits",
                    "ZWO CCD ASI294MM Pro", focus_position, image_number
                );
                File::create(filename)
                    .expect("Unable to create file")
                    .write_all(&image)
                    .expect("Unable to write file");
            }
            Ok(notify::Status::Pending)
        })
        .expect("Aquiring images");
}
