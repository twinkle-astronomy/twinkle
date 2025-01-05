use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::time::Duration;

use indi::TypeError;
use tokio::net::TcpStream;
use twinkle_client::notify::wait_fn;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let client = indi::client::new(TcpStream::connect(&args[1]).await.unwrap(), None, None);
    let camera = client
        .get_device::<()>("ZWO CCD ASI294MM Pro")
        .await
        .unwrap();
    camera
        .enable_blob(None, indi::BlobEnable::Also)
        .await
        .expect("enabling image delivery");
    let imager = camera.get_parameter("CCD1").await.unwrap();

    let mut image_number = 0;
    let mut imager_gen = imager.lock().await.gen();

    wait_fn::<(), TypeError, _, _>(imager.subscribe().await, Duration::MAX, |imager| {
        if imager.gen() == imager_gen {
            return Ok(twinkle_client::notify::Status::Pending);
        }
        imager_gen = imager.gen();

        let ccd = imager
            .get_values::<HashMap<String, indi::Blob>>()?
            .get("CCD1")
            .unwrap();
        if let Some(image) = &ccd.value {
            image_number += 1;
            let filename = format!(
                "{} {:02}.fits",
                "ZWO CCD ASI294MM Pro", image_number
            );
            File::create(filename)
                .expect("Unable to create file")
                .write_all(&image)
                .expect("Unable to write file");
        }
        Ok(twinkle_client::notify::Status::Pending)
    })
    .await
    .expect("Aquiring images");
}
