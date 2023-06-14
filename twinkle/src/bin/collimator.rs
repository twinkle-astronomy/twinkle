#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, net::TcpStream, ops::Deref, sync::Arc, thread};

use egui::mutex::Mutex;
use fits_inspect::{
    analysis::Statistics,
    egui::{fits_render::Circle, FitsRender, FitsWidget},
};
use fitsio::FitsFile;
use indi::client::{device::FitsImage, ClientConnection};
use ndarray::ArrayD;
use tokio::runtime::Runtime;
use tokio_stream::StreamExt;

use std::f64::consts::PI;

use fits_inspect::egui::fits_render::Elipse;

#[derive(PartialEq, Debug, Clone)]
enum Algo {
    DefocusedStar,
    PeakOffset,
}

#[derive(Debug, Clone, PartialEq)]
struct Settings {
    center_radius: f32,
    image: Arc<ArrayD<u16>>,
    defocused: fits_inspect::analysis::collimation::defocused_star::DefocusedStar,
    sep_threshold: f32,
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
                    sep_threshold: (2.0 as f32).powf(11.0),
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
                match sub.next().await {
                    Some(Ok(settings)) => {
                        dbg!("loop!");
                        let center = Circle {
                            x: image.shape()[1] as f32 / 2.0,
                            y: image.shape()[0] as f32 / 2.0,
                            r: settings.center_radius * max_radius(&image) / 2.0,
                        };
                        let stats = Statistics::new(&settings.image.view());
                        match settings.algo {
                            Algo::DefocusedStar => {
                                let mut fits_widget = calc_render.lock();
                                let circles = {
                                    fits_widget.set_fits(settings.image.clone());
                                    settings.defocused.calculate(&settings.image)
                                }
                                .chain([center.into()]);

                                fits_widget.set_elipses(circles);
                            }
                            Algo::PeakOffset => {
                                let image = settings.image.deref().clone();
                                let mut sep_image =
                                    fits_inspect::analysis::sep::Image::new(image).unwrap();
                                let bkg = sep_image.background().unwrap();
                                sep_image.sub(&bkg).expect("Subtract background");

                                let stars: Vec<fits_inspect::analysis::sep::CatalogEntry> =
                                    sep_image
                                        .extract(Some(settings.sep_threshold))
                                        .unwrap_or(vec![])
                                        .into_iter()
                                        .filter(|x| x.flag == 0)
                                        .filter(|x| x.peak * 1.2 < stats.clip_high.value as f32)
                                        .collect();

                                let mut star_iter = stars.iter();
                                let ((x, y), (xpeak, ypeak)) = if let Some(first) = star_iter.next()
                                {
                                    star_iter.fold(
                                        (
                                            (first.x, first.y),
                                            (first.xpeak as f64, first.ypeak as f64),
                                        ),
                                        |((x, y), (xpeak, ypeak)), star| {
                                            (
                                                (x + star.x, y + star.y),
                                                (
                                                    xpeak + star.xpeak as f64,
                                                    ypeak + star.ypeak as f64,
                                                ),
                                            )
                                        },
                                    )
                                } else {
                                    ((0.0, 0.0), (0.0, 0.0))
                                };
                                let ((x, y), (xpeak, ypeak)) = (
                                    (x / stars.len() as f64, y / stars.len() as f64),
                                    (xpeak / stars.len() as f64, ypeak / stars.len() as f64),
                                );

                                let centers = [
                                    Elipse {
                                        x: x as f32,
                                        y: y as f32,
                                        a: 0.5,
                                        b: 0.5,
                                        theta: 0.0,
                                    },
                                    Elipse {
                                        x: x as f32,
                                        y: y as f32,
                                        a: 0.5,
                                        b: 10.5,
                                        theta: 0.0,
                                    },
                                    Elipse {
                                        x: xpeak as f32,
                                        y: ypeak as f32,
                                        a: 10.5,
                                        b: 0.5,
                                        theta: 0.0,
                                    },
                                ];

                                dbg!(&centers);
                                let stars = stars
                                    .into_iter()
                                    .flat_map(|x| {
                                        let center1 = Elipse {
                                            x: x.x as f32,
                                            y: x.y as f32,
                                            a: 0.5,
                                            b: 0.5,
                                            theta: 0.0,
                                        };
                                        let center2 = Elipse {
                                            x: x.xpeak as f32,
                                            y: x.ypeak as f32,
                                            a: 0.5,
                                            b: 0.5,
                                            theta: 0.0,
                                        };
                                        [x.into(), center1, center2]
                                    })
                                    .chain(centers);
                                let mut fits_widget = calc_render.lock();
                                fits_widget.set_fits(settings.image.clone());
                                fits_widget.auto_stretch(&stats);
                                fits_widget.set_elipses(stars.chain([center.into()]));
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
            dbg!("end!");
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
                            &mut settings.sep_threshold,
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
