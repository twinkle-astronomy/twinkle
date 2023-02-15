use std::{net::TcpStream, env,  collections::HashMap};

use fits_inspect::analysis::Statistics;
use indi::*;
use fitsio::{ FitsFile, images::{ImageType}};

fn main() -> Result<(), ChangeError<()>> {
    let args: Vec<String> = env::args().collect();
    let addr = &args[1];
    let camera_name = &args[2];

    let client = indi::Client::new(TcpStream::connect(addr).expect(format!("Unable to connect to {}", addr).as_str()), None, None)?;
    let camera = client.get_device(&camera_name)?;

    camera.change("CONNECTION", vec![
        ("CONNECT", true )
    ])?()?;

    camera.enable_blob(Some("CCD1"), indi::BlobEnable::Also).unwrap();
    
    indi::batch(vec![
        camera.change("CCD_CONTROLS", vec![
            ( "Offset", 10.0 ),
            ( "Gain", 240.0 ),
        ])?,

        camera.change("FITS_HEADER", vec![
            ("FITS_OBJECT", ""),
        ])?,

        camera.change("CCD_BINNING", vec![
            ( "HOR_BIN", 2.0 ),
            ( "VER_BIN", 2.0 ),
        ])?,

        camera.change("CCD_FRAME_TYPE", vec![
            ( "FRAME_FLAT", true ),
        ])?,

    ])?;

    let efw = client.get_device("ASI EFW")?;

    efw.change("CONNECTION", vec![
        ("CONNECT", true )
    ])?()?;

    let filter_names: HashMap<String, f64> = {
        let filter_names_param = efw.get_parameter("FILTER_NAME")?;
        let l = filter_names_param.lock();
        l.get_values::<HashMap<String, Text>>()?.iter().map(|(slot, name)| {
            let slot = slot.split("_").last().map(|x| x.parse::<f64>().unwrap() ).unwrap();
            (name.value.clone(), slot)
        }).collect()
    };
    // dbg!(&filter_names);

    efw.change("FILTER_SLOT", vec![
        ( "FILTER_SLOT_VALUE", filter_names["Luminance"] ),
    ])?()?;
    
    let mut exposure = 1.0;
    let mut captured = 0;
    loop {

        println!("Exposing for {}s", exposure);
        let image_data = camera.capture_image(exposure).expect("Capturing image");
        let stats = Statistics::new(&image_data.view());

        dbg!(&stats.median);
        
        let target_median = u16::MAX / 2;
        if stats.median as f32 > 0.8 * 2.0_f32.powf(16.0) {
            exposure = exposure / 2.0;
        } else if (stats.median as f32) < {0.1 * 2.0_f32.powf(16.0)} {
            exposure = exposure * 2.0;
        } else if target_median.abs_diff(stats.median) > 1000 {
            exposure = (target_median as f64) / (stats.median as f64) * exposure;
        } else {
            captured += 1;

            let mut fptr = FitsFile::create(format!("Flat_{}.fits", captured)).open().unwrap();
            let mut hdu = fptr.primary_hdu().unwrap();
            if let fitsio::hdu::HduInfo::ImageInfo{ shape: _, image_type} = &mut hdu.info {
                *image_type = ImageType::UnsignedShort;
            }
            let hdu = hdu.resize(&mut fptr, image_data.shape()).unwrap();
            
            let slice = image_data.as_slice().unwrap();
            hdu.write_image(&mut fptr, slice).unwrap();

            println!("Finished: {}", captured);
        }
        if captured > 10 {
            break;
        }
    }
    Ok(())

    // exposure = find exposure med between 1k, 50k
    // correct_exposure_time = target / exposure_med * exposure_time



}