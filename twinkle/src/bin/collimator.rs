#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, net::TcpStream, sync::Arc, thread};

use egui::mutex::Mutex;
use fits_inspect::{
    egui::{FitsRender, FitsWidget, fits_render::Circle},
};
use fitsio::FitsFile;
use indi::client::{device::FitsImage, ClientConnection};
use ndarray::ArrayD;
use tokio::runtime::Runtime;
use tokio_stream::StreamExt;


use std::f64::consts::PI;

use fits_inspect::egui::fits_render::Elipse;
use opencv::{
    self,
    core::BORDER_CONSTANT,
    imgproc::{
        morphology_default_border_value, CHAIN_APPROX_NONE, MORPH_ELLIPSE, RETR_LIST,
        THRESH_BINARY,
    },
    prelude::Mat,
};

#[derive(Debug, Clone)]
pub struct DefocusedStar {
    /// Amount to blur.  Real value passed to opencv will be blur*2 + 1
    blur: i32,
    threshold: f64,
}

impl Default for DefocusedStar {
    fn default() -> Self {
        Self { blur: 7, threshold: 40.0 }
    }
}

impl DefocusedStar {

    fn calculate(&self, data: &ArrayD<u16>) -> Box<dyn Iterator<Item = Elipse>> {
        let raw_data: Vec<u8> = data.iter().map(|x| (*x >> 8) as u8).collect();
        let image = Mat::from_slice_rows_cols(&raw_data, data.shape()[0], data.shape()[1]).unwrap();

        let output: Mat = image.clone();

        let input: Mat = Default::default();
        let (input, mut output) = (output, input);

        opencv::imgproc::median_blur(&input, &mut output, self.blur*2+1).unwrap();
        let (input, mut output) = (output, input);

        opencv::imgproc::threshold(&input, &mut output, self.threshold, 255.0, THRESH_BINARY).unwrap();
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
                    let radius = (4.0 * area / PI).sqrt() / 2.0;

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
}

#[derive(PartialEq, Debug, Clone)]
enum Algo {
    DefocusedStar,
    PeakOffset,
}

#[derive(Debug, Clone)]
struct Settings {
    center_radius: f32,
    image: Arc<ArrayD<u16>>,
    defocused: DefocusedStar,
    algo: Algo
}
pub struct FitsViewerApp {
    fits_widget: Arc<Mutex<FitsRender>>,
    settings: Arc<indi::client::notify::Notify<Settings>>,
}

fn max_radius(image: &ArrayD<u16>) -> f32 {
    let (w, h) = (image.shape()[1], image.shape()[0]);
    ((w as f32).powi(2) + (h as f32).powi(2)).sqrt()
}

impl FitsViewerApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        let gl = cc.gl.as_ref()?;

        let mut fptr = FitsFile::open(
            "file.fits",
        )
        .unwrap();
        let hdu = fptr.primary_hdu().unwrap();
        let image: Arc<ArrayD<u16>> = Arc::new(hdu.read_image(&mut fptr).unwrap());

        let newed = FitsViewerApp {
            fits_widget: Arc::new(Mutex::new(FitsRender::new(gl))),
            settings: Arc::new(indi::client::notify::Notify::new_with_size(Settings{
                center_radius: 0.02,
                image: image.clone(),
                defocused: Default::default(),
                algo: Algo::DefocusedStar,
            }, 1)),
        };


        let settings = newed.settings.clone();

        let mut sub = settings.subscribe().unwrap();
        
        let calc_render = newed.fits_widget.clone();
        let calc_context = cc.egui_ctx.clone();
        tokio::spawn(async move {
            while let Some(Ok(settings)) = sub.next().await {
              
                let mut fits_widget = calc_render.lock();
                let circles = {
                    fits_widget.set_fits(settings.image.clone());
                    settings.defocused.calculate(&settings.image)
                }.chain(
                    [
                        Circle { 
                            x: image.shape()[1] as f32 / 2.0, y: image.shape()[0] as f32 / 2.0, r: settings.center_radius * max_radius(&image) / 2.0
                    }.into()]
                );
                
                fits_widget.set_elipses(circles);
            
                calc_context.request_repaint();
            }
        });
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
                        let data: Arc<ArrayD<u16>> = Arc::new(fits.read_image().expect("Reading captured image"));
                        
                        let mut lock = settings.lock().unwrap();
                        lock.image = data;
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
        egui::SidePanel::left("control").show(ctx, |ui| {
            let mut settings = self.settings.lock().unwrap();
            ui.add(
                egui::Slider::new(&mut settings.center_radius, 0.0..=1.0)
                    .text("Center Circle")
                    .show_value(false)
                    .logarithmic(true)
            );

            ui.horizontal(|ui| {
                ui.selectable_value(&mut settings.algo, Algo::DefocusedStar, "Defocused Star");
                ui.selectable_value(&mut settings.algo, Algo::PeakOffset, "Peak Offset");    
            });
            match settings.algo {
                Algo::DefocusedStar => {
                    ui.add(egui::Slider::new(&mut settings.defocused.blur, 0..=20).text("Blur"));
                    ui.add(egui::Slider::new(&mut settings.defocused.threshold, 0.0..=100.0).text("Threshold"));
                },
                Algo::PeakOffset => {

                }
            }

        });

        egui::CentralPanel::default().show(ctx, move |ui| {
            ui.add(FitsWidget::new(fits_widget));
        });
    }
}

fn main() {

    let rt = Runtime::new().expect("Unable to create Runtime");

    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Collimator",
        native_options,
        Box::new(move |cc| Box::new(FitsViewerApp::new(cc).unwrap())),
    );
}
