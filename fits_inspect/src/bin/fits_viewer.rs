#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, net::TcpStream, sync::Arc, thread, path::PathBuf};

use egui::mutex::Mutex;
use fits_inspect::{
    analysis::Statistics,
    egui::{fits_render::Elipse, FitsRender, FitsWidget}, Image, calibration::{CalibrationStore, CanCalibrate, HasCalibration}, HasImage,
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
        
        // let mut callibrations: CalibrationStore<fits_inspect::calibration::Image> = CalibrationStore::default();
        // let paths = std::fs::read_dir("/home/cconstantine/AstroDMx_DATA/twinkle/calibration/").unwrap();
    
        // for path in paths {
        //     let path = path.unwrap();
        //     println!("Name: {}", path.path().display());
        //     let image: Result<fits_inspect::calibration::Image, _> = path.path().try_into();
        //     match image {
        //         Ok(image) => {
        //             dbg!(&image.desc);
        //             callibrations.insert(image.desc.clone(), image);
        //         }
        //         Err(e) => {
        //             dbg!(e);
        //         }
        //     }
        // }
        let mut image: Image = PathBuf::from("~/AstroDMx_DATA/ekos/NGC_6992_mosaic/data/NGC_6992/NGC_6992-Part_1/Light/H_Alpha/NGC_6992-Part_1_Light_H_Alpha_720_secs_2023-06-21T00-54-28_010.fits").try_into().unwrap();
        dbg!(image.describe_dark());
        let dark: fits_inspect::calibration::Image = PathBuf::from("/home/cconstantine/AstroDMx_DATA/twinkle/calibration/masterDark_BIN-2_4144x2822_EXPOSURE-720.00s_GAIN-240.fit").try_into().unwrap();
        let flat: fits_inspect::calibration::Image = PathBuf::from("/home/cconstantine/AstroDMx_DATA/twinkle/calibration/masterFlat_BIN-2_4144x2822_FILTER-H-Alpha_mono.fit").try_into().unwrap();
        image.calibrate(&dark, &flat).ok();
        let data = image.get_data();
        {
            let mut lock = newed.fits_widget.lock();
            lock.set_fits(data);
            lock.auto_stretch(image.get_statistics());
        }
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
