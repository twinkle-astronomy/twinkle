use fits_inspect::egui::{FocusGraph};
use fitsio;

use ndarray::{ArrayD};
use std::sync::{Mutex, Arc};
use std::{env};
use fits_inspect::analysis::{sep, Star};
use std::fs;
use std::path::Path;



fn load_focus_events<T: AsRef<Path>>(directory: &T, focus_graph: Arc<Mutex<FocusGraph>>, ctx: egui::Context)  {
    let paths = fs::read_dir(directory).unwrap();
    for path in paths {
        let path = path.unwrap();
        let path_string = path.file_name().into_string().unwrap();
        println!("Loading: {}", path.file_name().into_string().unwrap());
        let splits: Vec<&str> = path_string.rsplit(|x| x == ' ' || x == '.').collect();

        if splits.len() > 3 {
            let focuser_position = splits[2].parse::<i32>().unwrap();
            
            let mut fptr = fitsio::FitsFile::open(path.path()).unwrap();
            let hdu = fptr.primary_hdu().unwrap();
            let image : ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();

            let sep_image = sep::Image::new(image).unwrap();
            let bkg = sep_image.background().unwrap();
            let catalog = sep_image.extract(&bkg).unwrap();


            println!("Found: {} stars", catalog.len());
            let fwhm = catalog.iter().map(|e| e.fwhm()).sum::<f32>() / catalog.len() as f32;
            {
                let mut focus_graph = focus_graph.lock().unwrap();
                focus_graph.add_point([focuser_position as f64, fwhm as f64]);
                ctx.request_repaint();
            }
        }
    }
}

pub struct FocusTestApp {
    focus_graph: Arc<Mutex<FocusGraph>>,
}

impl FocusTestApp {
    /// Called once before the first frame.
    fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {

        let args: Vec<String> = env::args().collect();

        let newed: FocusTestApp = FocusTestApp {
            focus_graph: Arc::new(Mutex::new(FocusGraph::new(cc)))
        };

        let focus_graph = newed.focus_graph.clone();
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            load_focus_events(&args[1], focus_graph, ctx);
        });
        Some(newed)
    }
}

impl eframe::App for FocusTestApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, move |_ui| {
            let mut focus_graph = self.focus_graph.lock().unwrap();
            focus_graph.update(ctx, frame);
        });
    }
}
fn main() {

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Focus Test",
        native_options,
        Box::new(move |cc| Box::new(FocusTestApp::new(cc).unwrap())),
    );

}
