#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, net::TcpStream, sync::Arc, thread};

use egui::mutex::Mutex;
use fits_inspect::{
    analysis::Statistics,
    egui::{FitsRender, FitsWidget, fits_render::Elipse},
};
use fitsio::FitsFile;
use indi::client::{device::FitsImage, ClientConnection};
use ndarray::ArrayD;

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
                // "~/test/test27.fits"
                "~/AstroDMx_DATA/ekos/NGC_6543/Light/H_Alpha/NGC_6543_Light_H_Alpha_720_secs_2023-06-10T23-03-41_009.fits",
            )
            .unwrap();
            let hdu = fptr.primary_hdu().unwrap();
            let data: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();
            let stats = Statistics::new(&data.view());

            let mut sep_image = fits_inspect::analysis::sep::Image::new(data.clone()).unwrap();
            let bkg = sep_image.background().unwrap();
            sep_image.sub(&bkg).expect("Subtract background");
            
            let stars: Vec<fits_inspect::analysis::sep::CatalogEntry> = sep_image.extract(None).unwrap()
                .into_iter()
                .filter(|x| x.flag == 0)
                .filter(|x| x.peak * 1.2 < stats.clip_high.value as f32).collect();

            let mut star_iter = stars.iter();
            let ((x, y), (xpeak, ypeak)) = if let Some(first) = star_iter.next() {
                star_iter.fold(((first.x, first.y), (first.xpeak as f64, first.ypeak as f64)), |((x, y), (xpeak, ypeak)), star| {
                    ((x + star.x, y + star.y), (xpeak + star.xpeak as f64, ypeak + star.ypeak as f64))
                })
            } else {
                todo!()
            };
            let ((x, y), (xpeak, ypeak)) = ((x / stars.len() as f64, y/ stars.len() as f64), (xpeak/ stars.len() as f64, ypeak/ stars.len() as f64));

            let centers = [
                Elipse { x: data.shape()[1] as f32 / 2.0, y: data.shape()[0] as f32/ 2.0, a: 20.0, b: 20.0, theta: 0.0 },
                Elipse { x: x as f32, y: y as f32, a: 0.5, b: 0.5, theta: 0.0 },
                Elipse { x: x as f32, y: y as f32, a: 0.5, b: 1.5, theta: 0.0 },
                Elipse { x: xpeak as f32, y: ypeak as f32, a: 1.5, b: 0.5, theta: 0.0 }
            ];

            dbg!(&centers);
            let stars = stars.into_iter()
                .flat_map(|x| {
                    let center1 = Elipse { x: x.x as f32, y: x.y as f32, a: 0.5, b: 0.5, theta: 0.0 };
                    let center2 = Elipse { x: x.xpeak as f32, y: x.ypeak as f32, a: 0.5, b: 0.5, theta: 0.0 };
                    [
                        x.into(),
                        center1,
                        center2,
                    ]
                }).chain(
                    centers
                );

            lock.set_fits(Arc::new(data));
            lock.set_elipses(stars);
            lock.auto_stretch(&stats);
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
                        let stats = Statistics::new(&data.view());

                        let mut sep_image =
                            fits_inspect::analysis::sep::Image::new(data.clone()).unwrap();
                        let bkg = sep_image.background().unwrap();
                        sep_image.sub(&bkg).expect("Subtract background");
                        let stars = sep_image.extract(None).unwrap();

                        {
                            let mut fits_widget = fits_widget.lock();
                            fits_widget.set_fits(Arc::new(data));
                            fits_widget.set_elipses(stars.into_iter().filter(|x| x.flag == 0));
                            fits_widget.auto_stretch(&stats);
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
