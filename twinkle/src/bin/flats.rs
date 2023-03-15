use std::{collections::HashMap, env, net::TcpStream};

use fits_inspect::analysis::Statistics;
use indi::*;
use twinkle::*;
use tokio::signal;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let addr = &args[1];

    let config = TelescopeConfig {
        mount: String::from("EQMod Mount"),
        imaging_optics: OpticsConfig {
            focal_length: 800.0,
            aperture: 203.0,
        },
        imaging_camera: String::from("ZWO CCD ASI294MM Pro"),
        focuser: String::from("ASI EAF"),
        filter_wheel: String::from("ASI EFW"),
    };

    let client = indi::client::new(
        TcpStream::connect(addr).expect(format!("Unable to connect to {}", addr).as_str()),
        None,
        None,
    ).expect("Connecting to INDI server");

    let camera = client
        .get_device(&config.imaging_camera)
        .await
        .expect("Getting imaging camera");

    camera
        .change("CONNECTION", vec![("CONNECT", true)])
        .await
        .expect("Connecting to camera");



    let image_client = indi::client::new(
        TcpStream::connect(addr).expect(format!("Unable to connect to {}", addr).as_str()),
        Some(&config.imaging_camera.clone()),
        Some("CCD1"),
    ).expect("Connecting to INDI server");
    let image_camera = image_client
        .get_device(&config.imaging_camera)
        .await
        .expect("Getting imaging camera");
    image_camera
        .enable_blob(Some("CCD1"), indi::BlobEnable::Only)
        .await
        .expect("enabling image retrieval");
    let image_ccd = image_camera.get_parameter("CCD1").await.unwrap();

    tokio::try_join!(
        camera.change("CCD_CAPTURE_FORMAT", vec![("ASI_IMG_RAW16", true)]),
        camera.change("CCD_TRANSFER_FORMAT", vec![("FORMAT_FITS", true)]),
        camera.change("CCD_CONTROLS", vec![("Offset", 10.0), ("Gain", 240.0)]),
        camera.change("FITS_HEADER", vec![("FITS_OBJECT", "")]),
        camera.change("CCD_BINNING", vec![("HOR_BIN", 2.0), ("VER_BIN", 2.0)]),
        camera.change("CCD_FRAME_TYPE", vec![("FRAME_FLAT", true)]),
    )
    .expect("Configuring camera");

    let filter_wheel = client
        .get_device(&config.filter_wheel)
        .await
        .expect("Getting filter wheel");

    filter_wheel
        .change("CONNECTION", vec![("CONNECT", true)])
        .await
        .expect("wating on change");

    let filter_names: HashMap<String, f64> = {
        let filter_names_param = filter_wheel
            .get_parameter("FILTER_NAME")
            .await
            .expect("getting filter names");
        let l = filter_names_param.lock().unwrap();
        l.get_values::<HashMap<String, Text>>()
            .expect("getting values")
            .iter()
            .map(|(slot, name)| {
                let slot = slot
                    .split("_")
                    .last()
                    .map(|x| x.parse::<f64>().unwrap())
                    .unwrap();
                (name.value.clone(), slot)
            })
            .collect()
    };

    filter_wheel
        .change(
            "FILTER_SLOT",
            vec![("FILTER_SLOT_VALUE", filter_names["Luminance"])],
        )
        .await
        .expect("Changing filter");

    let mut exposure = 1.0;
    let mut captured = 0;

    let task = tokio::spawn( async move {
        loop {
            println!("Exposing for {}s", exposure);
            let fits_data =    camera
                .capture_image_from_param(exposure, &image_ccd)
                .await
                .expect("Capturing image");

            let image_data = fits_data.read_image().expect("Reading captured image");
            let stats = Statistics::new(&image_data.view());

            dbg!(&stats.median);

            let target_median = u16::MAX / 2;
            if stats.median as f32 > 0.8 * 2.0_f32.powf(16.0) {
                exposure = exposure / 2.0;
            } else if (stats.median as f32) < { 0.1 * 2.0_f32.powf(16.0) } {
                exposure = exposure * 2.0;
            } else if target_median.abs_diff(stats.median) > 1000 {
                exposure = (target_median as f64) / (stats.median as f64) * exposure;
            } else {
                captured += 1;
                fits_data
                    .save(format!("Flat_{}.fits", captured))
                    .expect("saving fits file");

                println!("Finished: {}", captured);
            }
            if captured > 0 {
                break;
            }
        }
    });
    
    tokio::select! {
        _ = signal::ctrl_c() => {
            println!("Aborting");
            return;
        },
        _ = task => {
            
        }
    };

    // exposure = find exposure med between 1k, 50k
    // correct_exposure_time = target / exposure_med * exposure_time
}
