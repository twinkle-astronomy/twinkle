use std::sync::mpsc::{self, Receiver, Sender};
use std::{collections::HashMap, env, f64::consts::PI, net::TcpStream, time::Instant};

use fitsio::FitsFile;
use indi::client::device::FitsImage;
use ndarray::ArrayD;
use opencv::{
    self,
    core::BORDER_CONSTANT,
    imgproc::{
        morphology_default_border_value, CHAIN_APPROX_NONE, LINE_8, MORPH_ELLIPSE, RETR_LIST,
        THRESH_BINARY,
    },
    prelude::Mat,
};
use tokio_stream::StreamExt;

fn new_image(data: &ArrayD<u16>) {
    let raw_data: Vec<u8> = data.iter().map(|x| (*x >> 8) as u8).collect();
    let mut image = Mat::from_slice_rows_cols(&raw_data, data.shape()[0], data.shape()[1]).unwrap();

    // let mut image = opencv::imgcodecs::imread("file.png", opencv::imgcodecs::IMREAD_COLOR)
    //     .expect("Reading file");

    let output: Mat = image.clone();
    // opencv::imgproc::cvt_color(&image, &mut output, COLOR_BGR2GRAY, 0).unwrap();

    let input: Mat = Default::default();
    let (input, mut output) = (output, input);

    let start = Instant::now();
    opencv::imgproc::median_blur(&input, &mut output, 15).unwrap();
    let (input, mut output) = (output, input);

    opencv::imgproc::threshold(&input, &mut output, 40.0, 255.0, THRESH_BINARY).unwrap();
    let (input, mut output) = (output, input);

    let kernel =
        opencv::imgproc::get_structuring_element(MORPH_ELLIPSE, (5, 5).into(), (-1, -1).into())
            .unwrap();
    opencv::imgproc::morphology_ex(
        &input,
        &mut output,
        opencv::imgproc::MORPH_CLOSE,
        &kernel,
        (-1, -1).into(),
        1,
        BORDER_CONSTANT,
        morphology_default_border_value().unwrap(),
    )
    .unwrap();
    let (input, _) = (output, input);

    let mut contours: opencv::core::Vector<opencv::core::Vector<opencv::core::Point>> =
        Default::default();
    opencv::imgproc::find_contours(
        &input,
        &mut contours,
        RETR_LIST,
        CHAIN_APPROX_NONE,
        (0, 0).into(),
    )
    .unwrap();

    dbg!(start.elapsed());
    for contour in contours {
        let m = opencv::imgproc::moments(&contour, false).unwrap();
        if m.m00 == 0.0 {
            continue;
        }
        let area = opencv::imgproc::contour_area(&contour, false).unwrap();
        let dia = (4.0 * area / PI).sqrt();

        let cx = m.m10 / m.m00;
        let cy = m.m01 / m.m00;

        opencv::imgproc::circle(
            &mut image,
            (cx as i32, cy as i32).into(),
            (dia / 2.0) as i32,
            (255.0, 255.0, 255.0).into(),
            1,
            LINE_8,
            0,
        )
        .unwrap();

        opencv::imgproc::circle(
            &mut image,
            (cx as i32, cy as i32).into(),
            1 as i32,
            (255.0, 255.0, 255.0).into(),
            1,
            LINE_8,
            0,
        )
        .unwrap();
    }

    opencv::highgui::imshow("Collimation", &image).expect("show image");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let addr = &args[1];

    let client = indi::client::new(
        TcpStream::connect(addr).expect(format!("Unable to connect to {}", addr).as_str()),
        None,
        None,
    )
    .expect("Connecting to indi");

    let (tx, rx): (Sender<ArrayD<u16>>, Receiver<ArrayD<u16>>) = mpsc::channel();

    tokio::task::spawn_blocking(move || {
        opencv::highgui::named_window("Collimation", 0).expect("Open window");
        opencv::highgui::start_window_thread().unwrap();

        let mut fptr = FitsFile::open("file.fits").unwrap();
        let hdu = fptr.primary_hdu().unwrap();

        let data: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();
        new_image(&data);

        while opencv::highgui::get_window_property("Collimation", 0).unwrap() >= 0.0 {
            if let Ok(v) = rx.try_recv() {
                new_image(&v);
            }
            let key = opencv::highgui::wait_key(10).unwrap();
            if key == 113 {
                std::process::exit(0);
            }
        }
    });

    let camera = client
        .get_device::<()>("ZWO CCD ASI294MM Pro")
        .await
        .unwrap();
    camera
        .enable_blob(Some("CCD1"), indi::BlobEnable::Only)
        .await
        .unwrap();

    let mut ccd = camera.get_parameter("CCD1").await.unwrap().changes();

    while let Some(Ok(image_param)) = ccd.next().await {
        if let Some(image_data) = image_param
            .get_values::<HashMap<String, indi::Blob>>()
            .unwrap()
            .get("CCD1")
        {
            if let Some(bytes) = &image_data.value {
                let fits_image = FitsImage::new(bytes.clone());
                let data = fits_image.read_image().unwrap();
                tx.send(data).unwrap();
            } else {
                dbg!("No image data");
            }
        }
    }
}
