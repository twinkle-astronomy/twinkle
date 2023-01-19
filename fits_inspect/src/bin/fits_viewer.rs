#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::env;

use fits_inspect::egui::FitsWidget;
use fitsio::FitsFile;

pub struct FitsViewerApp {
    fits_widget: Option<FitsWidget>
}

impl Default for FitsViewerApp {
    fn default() -> Self {
        Self {
            fits_widget: None,
        }
    }
}

impl FitsViewerApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {

        let mut newed: FitsViewerApp = Default::default();
        newed.fits_widget = FitsWidget::new(cc);
        Some(newed)
    }
}

impl eframe::App for FitsViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        egui::CentralPanel::default().show(ctx, |_ui| {
            if let Some(fits_widget) = &mut self.fits_widget {
                fits_widget.update(ctx, _frame);
            }
        });
    }
}


fn main() {
    // tracing_subscriber::fmt::init();
    let args: Vec<String> = env::args().collect();

    let filename = &args[1];


    let fptr = FitsFile::open(filename).unwrap();

    
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Fits Viewer",
        native_options,
        Box::new(|cc| Box::new({
            let mut app = FitsViewerApp::new(cc).unwrap();
            if let Some(ref mut w) = app.fits_widget {

                let gl = cc.gl.as_ref().unwrap();
                w.set_fits(gl, fptr);
            }
            app
        })),
    );
}
