#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, net::TcpStream, sync::Arc, thread};

use egui::mutex::Mutex;
use fits_inspect::{
    analysis::Statistics,
    egui::{FitsRender, FitsWidget},
};
use fitsio::FitsFile;
use indi::client::{device::FitsImage, ClientConnection};
use ndarray::ArrayD;

use std::sync::mpsc::{self, Receiver, Sender};
use std::{collections::HashMap, f64::consts::PI, time::Instant};

use fits_inspect::egui::fits_render::Elipse;
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

fn new_image(data: &ArrayD<u16>) -> Box<dyn Iterator<Item = Elipse>> {
    let raw_data: Vec<u8> = data.iter().map(|x| (*x >> 8) as u8).collect();
    let image = Mat::from_slice_rows_cols(&raw_data, data.shape()[0], data.shape()[1]).unwrap();

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
    return Box::new(
        contours
            .into_iter()
            .map(|contour| {
                let m = opencv::imgproc::moments(&contour, false).unwrap();
                (contour, m)
            })
            .filter(|(_contour, m)| m.m00 != 0.0)
            .flat_map(|(contour, m)| {
                let area = opencv::imgproc::contour_area(&contour, false).unwrap();
                let radius = (4.0 * area / PI).sqrt() / 4.0;

                [
                    Elipse {
                        x: (m.m10 / m.m00) as f32,
                        y: (m.m01 / m.m00) as f32,
                        a: radius as f32,
                        b: radius as f32,
                        theta: 0.0,
                    },
                    Elipse {
                        x: (m.m10 / m.m00) as f32,
                        y: (m.m01 / m.m00) as f32,
                        a: 1.0 as f32,
                        b: 1.0 as f32,
                        theta: 0.0,
                    },
                ]
            }),
    );
}

pub struct FitsViewerApp {
    fits_widget: Arc<Mutex<FitsRender>>,
}

impl FitsViewerApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        let gl = cc.gl.as_ref()?;

        let newed = FitsViewerApp {
            fits_widget: Arc::new(Mutex::new(FitsRender::new(gl))),
        };

        {
            let mut lock = newed.fits_widget.lock();
            let mut fptr = FitsFile::open(
                "file.fits",
            )
            .unwrap();
            let hdu = fptr.primary_hdu().unwrap();
            let data: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();

            let circles = new_image(&data);
            
            lock.set_fits(data);
            lock.set_elipses(circles);
        }

        let fits_widget = newed.fits_widget.clone();
        let ctx = cc.egui_ctx.clone();
        thread::spawn(move || {
            let args: Vec<String> = env::args().collect();

            let connection = TcpStream::connect(&args[1]).unwrap();
            connection
                .write(&indi::serialization::GetProperties {
                    version: indi::INDI_PROTOCOL_VERSION.to_string(),
                    device: None,
                    name: None,
                })
                .unwrap();

            connection
                .write(&indi::serialization::EnableBlob {
                    device: String::from("ZWO CCD ASI294MM Pro"),
                    name: None,
                    enabled: indi::BlobEnable::Only,
                })
                .unwrap();

            let c_iter = connection.iter().unwrap();

            for command in c_iter {
                match command {
                    Ok(indi::serialization::Command::SetBlobVector(mut sbv)) => {
                        println!("Got image for: {:?}", sbv.device);
                        if sbv.device != String::from("ZWO CCD ASI294MM Pro") {
                            continue;
                        }
                        let fits =
                            FitsImage::new(Arc::new(sbv.blobs.get_mut(0).unwrap().value.clone()));
                        let data: ArrayD<u16> = fits.read_image().expect("Reading captured image");
                        let circles = new_image(&data);
                        {
                            let mut fits_widget = fits_widget.lock();
                            fits_widget.set_fits(data);
                            fits_widget.set_elipses(circles);
                        }
                        ctx.request_repaint();
                    }
                    _ => {}
                }
            }
        });
        Some(newed)
    }
}

impl eframe::App for FitsViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let fits_widget = self.fits_widget.clone();
        egui::CentralPanel::default().show(ctx, move |ui| {
            ui.add(FitsWidget::new(fits_widget));
        });
    }
}

fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Fits Viewer",
        native_options,
        Box::new(move |cc| Box::new(FitsViewerApp::new(cc).unwrap())),
    );
}
