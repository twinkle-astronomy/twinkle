use std::{env, sync::Arc};

use egui::mutex::Mutex;
use fits_inspect::egui::{FitsRender, FitsWidget};

use tokio::runtime::Runtime;

use twinkle::{OpticsConfig, Telescope, TelescopeConfig};

pub struct FlatApp {
    fits_render: Arc<Mutex<FitsRender>>,
}

impl FlatApp {
    /// Called once before the first frame.
    fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        let args: Vec<String> = env::args().collect();
        let addr = &args[1];

        let config = TelescopeConfig {
            mount: String::from("EQMod Mount"),
            primary_optics: OpticsConfig {
                focal_length: 800.0,
                aperture: 203.0,
            },
            primary_camera: String::from("ZWO CCD ASI294MM Pro"),
            focuser: String::from("ASI EAF"),
            filter_wheel: String::from("ASI EFW"),
            flat_panel: String::from("Deep Sky Dad FP1"),
        };
        let telescope = Arc::new(Telescope::new(addr, config));
        let fits_render = Arc::new(Mutex::new(FitsRender::new(cc.gl.as_ref().unwrap())));
        let background_ctx = cc.egui_ctx.clone();
        let background_fits_render = fits_render.clone();
        let background_telescope = telescope.clone();

        tokio::spawn(async move {
            dbg!("1");
            let camera = background_telescope.get_primary_camera().await.unwrap();
            let camera_ccd = background_telescope.get_primary_camera_ccd().await.unwrap();
            loop {
                dbg!("2");
                let fits_data = camera
                    .next_image(&camera_ccd)
                    .await
                    .expect("Capturing image");

                let image_data = fits_data.read_image().expect("Reading captured image");

                // let stats = Statistics::new(&image_data.view());
                let mut fits_render = background_fits_render.lock();
                fits_render.set_fits(Arc::new(image_data));
                // fits_render.auto_stretch(&stats);
                background_ctx.request_repaint();
            }
        });

        let newed: FlatApp = FlatApp { fits_render };

        Some(newed)
    }
}

impl FlatApp {}

impl eframe::App for FlatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(FitsWidget::new(self.fits_render.clone()));
        });
    }
}

fn main() {
    let native_options = eframe::NativeOptions::default();
    let rt = Runtime::new().expect("Unable to create Runtime");

    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();
    eframe::run_native(
        "Viewer",
        native_options,
        Box::new(move |cc| Box::new(FlatApp::new(cc).unwrap())),
    );
}
