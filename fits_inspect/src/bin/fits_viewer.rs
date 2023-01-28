#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{
    env,
    sync::{Arc, Mutex},
    thread,
};

use fits_inspect::{analysis::Statistics, egui::FitsWidget};
use fitsio::FitsFile;
use ndarray::ArrayD;

pub struct FitsViewerApp {
    fits_widget: Arc<Mutex<Option<FitsWidget>>>,
}

impl Default for FitsViewerApp {
    fn default() -> Self {
        Self {
            fits_widget: Arc::new(Mutex::new(None)),
        }
    }
}

impl FitsViewerApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        let newed: FitsViewerApp = Default::default();
        {
            let mut fits_widget = newed.fits_widget.lock().unwrap();
            *fits_widget = FitsWidget::new(cc);
        }

        let fits_widget = newed.fits_widget.clone();
        let ctx = cc.egui_ctx.clone();
        thread::spawn(move || {
            // thread::sleep(time::Duration::from_millis(5000));
            // let mut fptr = FitsFile::open("images/M_33_Light_Red_180_secs_2022-11-24T18-58-20_001.fits").unwrap();
            // let hdu = fptr.primary_hdu().unwrap();
            // let image: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();

            // let mut fits_widget = fits_widget.lock().unwrap();
            // if let Some(w) = &mut *fits_widget {
            //     let stats = Statistics::new(&image.view());
            //     w.set_fits(image, stats);
            //     ctx.request_repaint();
            // }
            let args: Vec<String> = env::args().collect();

            let mut connection = indi::Connection::new(&args[1]).unwrap();
            connection
                .write(&indi::GetProperties {
                    version: indi::INDI_PROTOCOL_VERSION.to_string(),
                    device: None,
                    name: None,
                })
                .unwrap();

            connection
                .write(&indi::EnableBlob {
                    device: String::from("ZWO CCD ASI294MM Pro"),
                    name: None,
                    enabled: indi::BlobEnable::Only,
                })
                .unwrap();

            let c_iter = connection.iter().unwrap();

            for command in c_iter {
                match command {
                    Ok(indi::Command::SetBlobVector(mut sbv)) => {
                        println!("Got image for: {:?}", sbv.device);
                        if sbv.device != String::from("ZWO CCD ASI294MM Pro") {
                            continue;
                        }

                        let mut fptr =
                            FitsFile::open_memfile(&mut sbv.blobs.get_mut(0).unwrap().value)
                                .unwrap();
                        let hdu = fptr.primary_hdu().unwrap();
                        let data: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();

                        let mut fits_widget = fits_widget.lock().unwrap();
                        if let Some(w) = &mut *fits_widget {
                            let stats = Statistics::new(&data.view());
                            w.set_fits(data, stats);
                            ctx.request_repaint();
                        }
                    }
                    _ => {}
                }
            }
        });

        // let filename = &args[1];
        // let mut file_data = fs::read(filename).unwrap();
        // let mut fptr = FitsFile::open_memfile(&mut file_data).unwrap();
        // let hdu = fptr.primary_hdu().unwrap();
        // let data: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();

        // if let Some(ref mut w) = newed.fits_widget {
        //     let gl = cc.gl.as_ref().unwrap();
        //     w.set_fits(gl, data);
        // }
        Some(newed)
    }
}

impl eframe::App for FitsViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let fits_widget = self.fits_widget.clone();
        egui::CentralPanel::default().show(ctx, move |_ui| {
            let mut fits_widget = fits_widget.lock().unwrap();
            if let Some(w) = &mut *fits_widget {
                w.update(ctx, _frame);
            }
        });
    }
}

fn main() {
    // tracing_subscriber::fmt::init();

    // dbg!(data.len());
    // dbg!(file_data.len());
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Fits Viewer",
        native_options,
        Box::new(move |cc| Box::new(FitsViewerApp::new(cc).unwrap())),
    );
}
