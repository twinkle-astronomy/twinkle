use std::{
    collections::{BTreeMap, HashMap},
    env,
    sync::Arc,
};

use egui::{mutex::Mutex, ProgressBar};
use fits_inspect::{egui::{FitsRender, FitsWidget}, analysis::Statistics};
use indi::Number;
use tokio::runtime::Runtime;
use tokio_stream::StreamExt;

use ndarray::{Array2, ArrayD};
use twinkle::{
    flat::{self, SetConfig, Status},
    Action, OpticsConfig, Telescope, TelescopeConfig,
};

pub struct FlatApp {
    config: SetConfig,
    telescope: Arc<Telescope>,
    runner: Option<flat::Runner>,
    fits_render: Arc<Mutex<FitsRender>>,
    status: Arc<Mutex<Status>>,
}

impl FlatApp {
    /// Called once before the first frame.
    fn new(_cc: &eframe::CreationContext<'_>) -> Option<Self> {
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
        };
        let telescope = Arc::new(Telescope::new(addr, config));

        let flat_config = SetConfig {
            count: 10,
            filters: HashMap::default(),
            adu_target: 1000, //u16::MAX / 2,
            adu_margin: 1000,
            binnings: HashMap::default(),
            gain: 240.0,
            offset: 10.0,
        };

        let newed: FlatApp = FlatApp {
            config: flat_config,
            telescope,
            runner: None,
            fits_render: Arc::new(Mutex::new(FitsRender::new(
                _cc.gl.as_ref().unwrap(),
            ))),
            status: Arc::new(Mutex::new(Status::default())),
        };

        Some(newed)
    }
}

impl FlatApp {
    fn config_ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Grid::new("config")
            .num_columns(2)
            // .spacing([40.0, 40.0])
            .striped(false)
            .show(ui, |ui| {
                ui.label("Count");
                ui.add(egui::DragValue::new(&mut self.config.count).clamp_range(0u16..=u16::MAX));
                ui.end_row();

                ui.label("Filter");

                let filters = self.telescope.block_on(async {
                    let efw = self.telescope.get_filter_wheel().await.unwrap();

                    efw.change("CONNECTION", vec![("CONNECT", true)])
                        .await
                        .expect("Connecting to devices");

                    self.telescope
                        .get_filter_wheel()
                        .await
                        .unwrap()
                        .filter_names()
                        .await
                        .unwrap()
                });

                // Make sure we only have filters in the UI that exist
                self.config.filters.retain(|k, _v| filters.contains_key(k));

                let filters: BTreeMap<&usize, &String> =
                    filters.iter().map(|(k, v)| (v, k)).collect();
                for (_index, filter) in filters {
                    let entry = self
                        .config
                        .filters
                        .entry(filter.clone())
                        .or_insert_with(|| false);

                    ui.toggle_value(entry, filter);
                }
                ui.end_row();

                ui.label("Target ADU");
                ui.add(
                    egui::DragValue::new(&mut self.config.adu_target).clamp_range(0u16..=u16::MAX),
                );
                ui.end_row();

                ui.label("Binning");
                let bins = self.telescope.block_on(async {
                    let camera = self.telescope.get_primary_camera().await.unwrap();
                    let bin_param = camera.get_parameter("CCD_BINNING").await.unwrap();
                    let lock = bin_param.lock().unwrap();
                    let values = lock.get_values::<HashMap<String, Number>>().unwrap();
                    (values["HOR_BIN"].min as u8)..=(values["HOR_BIN"].max as u8)
                });
                for bin in bins {
                    let entry = self.config.binnings.entry(bin).or_insert_with(|| false);

                    ui.toggle_value(entry, format!("bin{}", bin)); //egui::DragValue::new(&mut self.config.binning).clamp_range(0..=8));
                }
                // ui.add(egui::DragValue::new(&mut self.config.binning).clamp_range(0..=8));
                ui.end_row();

                ui.label("Gain");
                ui.add(egui::DragValue::new(&mut self.config.gain).clamp_range(0..=500));
                ui.end_row();

                ui.label("Offset");
                ui.add(egui::DragValue::new(&mut self.config.offset).clamp_range(0..=500));
                ui.end_row();
            });
    }
}

impl eframe::App for FlatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("Left").show(ctx, |ui| {
            ui.vertical(|ui| {
                let running = self
                    .runner
                    .as_ref()
                    .map_or(false, |runner| !runner.task.is_finished());
                ui.add_enabled_ui(!running, |ui| {
                    self.config_ui(ui, ctx, _frame);
                });

                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!running, |ui| {
                        if ui.button("Run").clicked() {
                            let runner =
                                flat::Runner::new_set(self.config.clone(), self.telescope.clone());
                            let mut recv = runner.status();
                            let spawn_ctx = ctx.clone();
                            let fits_render = self.fits_render.clone();
                            let app_status = self.status.clone();
                            tokio::spawn(async move {
                                loop {
                                    match recv.next().await {
                                        Some(Ok(status)) => {
                                            {
                                                {
                                                    let mut lock = app_status.lock();
                                                    lock.complete = status.complete;
                                                }
                                                // let fits = status.image;
                                                if let Some(fits) = &status.image {
                                                    let data: ArrayD<u16> = fits
                                                        .read_image()
                                                        .expect("Reading captured imager");
                                                    let stats = Statistics::new(&data.view());
                                                    let mut fits_render = fits_render.lock();
                                                    fits_render.set_fits(data);
                                                    fits_render.auto_stretch(&stats);
                                                    spawn_ctx.request_repaint();
                                                }
                                            }
                                            // dbg!(status);
                                        }
                                        Some(Err(e)) => {
                                            dbg!(e);
                                        }
                                        None => {
                                            println!("Done");
                                            break;
                                        }
                                    }
                                }
                            });
                            self.runner = Some(runner);
                        }
                    });

                    ui.add_enabled_ui(running, |ui| {
                        if ui.button("Cancel").clicked() {
                            self.runner
                                .as_ref()
                                .and_then(|runner| Some(runner.task.abort()));
                            self.runner = None;
                        }
                        if running {
                            ui.spinner();
                        }
                    });
                });

                ui.add(ProgressBar::new(
                    (self.status.lock().complete as f32) / (self.config.expected_total() as f32),
                ));
            });
            // dbg!(&self.config);
        });

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
        "Flats",
        native_options,
        Box::new(move |cc| Box::new(FlatApp::new(cc).unwrap())),
    );
}
