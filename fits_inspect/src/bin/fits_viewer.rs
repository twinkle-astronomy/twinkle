#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, net::TcpStream, sync::Arc, thread};

use eframe::glow;
use egui::mutex::Mutex;
use fits_inspect::{
    analysis::Statistics,
    egui::{FitsRender, FitsWidget},
};
use fitsio::FitsFile;
use indi::client::{device::FitsImage, ClientConnection};
use ndarray::ArrayD;

pub struct FitsViewerApp {
    fits_widget: Arc<Mutex<FitsWidget>>,
}

impl FitsViewerApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        let mut fptr = FitsFile::open(
            "../fits_inspect/images/NGC_281_Light_Red_15_secs_2022-11-13T01-13-00_001.fits",
        )
        .unwrap();
        let hdu = fptr.primary_hdu().unwrap();
        let image: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();

        let gl = cc.gl.as_ref()?;

        let newed = FitsViewerApp {
            fits_widget: Arc::new(Mutex::new(FitsWidget::new(gl, image))),
        };

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
                        {
                            let mut fits_widget = fits_widget.lock();
                            fits_widget.set_fits(data);
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let fits_widget = self.fits_widget.clone();
        egui::CentralPanel::default().show(ctx, move |ui| {
            let mut lock = fits_widget.lock();
            lock.update(ctx, frame);
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
