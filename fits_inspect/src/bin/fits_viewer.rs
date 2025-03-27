#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{path::PathBuf, sync::Arc};

use egui::mutex::Mutex;
use fits_inspect::{
    // calibration::{CanCalibrate, HasCalibration},
    egui::{FitsRender, FitsWidget},
    HasImage,
    Image,
};

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
        let image: Image = PathBuf::from("/home/cconstantine/AstroDMx_DATA/ekos/NGC_2244/data/Light/H_Alpha/NGC_2244_Light_H_Alpha_360_secs_2025-02-25T21-56-53_003.fits").try_into().unwrap();
        // dbg!(image.describe_dark());
        // let dark: fits_inspect::calibration::Image = PathBuf::from("~/AstroDMx_DATA/ekos/NGC_2244/data/calibration/masterDark_BIN-2_4144x2822_EXPOSURE-360.00s_GAIN-120.xisf").try_into().unwrap();
        // let flat: fits_inspect::calibration::Image = PathBuf::from("~/AstroDMx_DATA/ekos/NGC_2244/data/calibration/masterFlat_BIN-2_4144x2822_FILTER-H-Alpha_mono.xisf").try_into().unwrap();
        // image.calibrate(&dark, &flat).ok();
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
