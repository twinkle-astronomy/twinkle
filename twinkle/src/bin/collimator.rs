#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, net::TcpStream, sync::Arc, thread};

use egui::mutex::Mutex;
use fits_inspect::{
    analysis::collimation::CollimationCalculator,
    egui::{fits_render::Circle, FitsRender, FitsWidget},
};
use fitsio::FitsFile;
use indi::client::{device::FitsImage, ClientConnection};
use ndarray::ArrayD;
use tokio::runtime::Runtime;
use tokio_stream::StreamExt;

#[derive(PartialEq, Debug, Clone)]
enum Algo {
    DefocusedStar,
    PeakOffset,
}

#[derive(Debug, Clone, PartialEq)]
struct Settings {
    center_radius: f32,
    image: Arc<ArrayD<u16>>,
    defocused: fits_inspect::analysis::collimation::DefocusedStar,
    peak_offset: fits_inspect::analysis::collimation::StarPeakOffset,
    algo: Algo,
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

        let mut fptr = FitsFile::open("~/test/test27.fits").unwrap();
        let hdu = fptr.primary_hdu().unwrap();
        let image: Arc<ArrayD<u16>> = Arc::new(hdu.read_image(&mut fptr).unwrap());

        let newed = FitsViewerApp {
            fits_widget: Arc::new(Mutex::new(FitsRender::new(gl))),
            settings: Arc::new(indi::client::notify::Notify::new_with_size(
                Settings {
                    center_radius: 0.02,
                    image: image.clone(),
                    defocused: Default::default(),
                    peak_offset: Default::default(),
                    algo: Algo::DefocusedStar,
                },
                1,
            )),
        };

        let settings = newed.settings.clone();

        let mut sub = settings.subscribe().unwrap();

        let calc_render = newed.fits_widget.clone();
        let calc_context = cc.egui_ctx.clone();
        tokio::spawn(async move {
            loop {
                dbg!("loop!");
                match sub.next().await {
                    Some(Ok(settings)) => {
                        let center = Circle {
                            x: image.shape()[1] as f32 / 2.0,
                            y: image.shape()[0] as f32 / 2.0,
                            r: settings.center_radius * max_radius(&image) / 2.0,
                        };
                        match settings.algo {
                            Algo::DefocusedStar => {
                                let circles = settings
                                    .defocused
                                    .calculate(&settings.image)
                                    .unwrap_or_else(|x| {
                                        dbg!(x);
                                        Box::new(vec![].into_iter())
                                    });

                                let mut fits_widget = calc_render.lock();
                                fits_widget.set_fits(settings.image.clone());
                                fits_widget.set_elipses(circles.chain([center.into()]));
                            }
                            Algo::PeakOffset => {
                                let circles = settings
                                    .peak_offset
                                    .calculate(&settings.image)
                                    .unwrap_or_else(|x| {
                                        dbg!(x);
                                        Box::new(vec![].into_iter())
                                    });

                                let mut fits_widget = calc_render.lock();
                                fits_widget.set_fits(settings.image.clone());
                                fits_widget.set_elipses(circles.chain([center.into()]));
                            }
                        }

                        calc_context.request_repaint();
                    }
                    Some(Err(e)) => {
                        dbg!(e);
                    }
                    None => {
                        dbg!("None");
                    }
                }
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
                            FitsImage::new(Arc::new(sbv.blobs.get_mut(0).unwrap().value.clone().into()));
                        let data: Arc<ArrayD<u16>> =
                            Arc::new(fits.read_image().expect("Reading captured image"));

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
            let old_settings = self.settings.lock().unwrap();
            let mut settings = old_settings.clone();

            ui.add(
                egui::Slider::new(&mut settings.center_radius, 0.0..=1.0)
                    .text("Center Circle")
                    .show_value(false)
                    .logarithmic(true)
                    .smallest_positive(0.01),
            );

            ui.horizontal(|ui| {
                ui.selectable_value(&mut settings.algo, Algo::DefocusedStar, "Defocused Star");
                ui.selectable_value(&mut settings.algo, Algo::PeakOffset, "Peak Offset");
            });
            ui.horizontal(|ui| {
                ui.add_space(220.0);
            });
            // ui.separator();
            match settings.algo {
                Algo::DefocusedStar => {
                    ui.add(egui::Slider::new(&mut settings.defocused.blur, 0..=20).text("Blur"));
                    ui.add(
                        egui::Slider::new(&mut settings.defocused.threshold, 0.0..=100.0)
                            .text("Threshold"),
                    );
                }
                Algo::PeakOffset => {
                    ui.add(
                        egui::Slider::new(
                            &mut settings.peak_offset.threshold,
                            0.0..=(std::u16::MAX as f32),
                        )
                        .text("SepThresh")
                        .logarithmic(true)
                        .smallest_positive(0.01),
                    );
                }
            }

            if *old_settings != settings {
                let mut old_settings = old_settings;
                *old_settings = settings;
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
