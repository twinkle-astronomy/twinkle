
use parking_lot::Mutex;

use crate::capture::CaptureManager;
use crate::flats::FlatManager;
use crate::indi::IndiManager;
use crate::settings::SettingsManager;
use std::boxed::Box;
use std::ops::DerefMut;
use std::sync::{mpsc, Arc, OnceLock};
static GLOBAL_CALLBACKS: OnceLock<
    std::sync::mpsc::Sender<Box<dyn FnOnce(&egui::Context, &mut eframe::Frame) + Sync + Send>>,
> = OnceLock::new();

pub struct App {
    indi_manager: Arc<Mutex<IndiManager>>,
    flats_manager: Arc<Mutex<FlatManager>>,
    settings_manager: Arc<Mutex<SettingsManager>>,
    capture_manager: Arc<Mutex<CaptureManager>>,

    callbacks: std::sync::mpsc::Receiver<
        Box<dyn FnOnce(&egui::Context, &mut eframe::Frame) + Sync + Send>,
    >,
}

impl App {
    pub fn run_next_update(
        func: Box<dyn FnOnce(&egui::Context, &mut eframe::Frame) + Sync + Send>,
    ) {
        if let Some(tx) = GLOBAL_CALLBACKS.get() {
            let _ = tx.send(func);
        }
    }
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, callbacks) = mpsc::channel();
        GLOBAL_CALLBACKS
            .set(tx)
            .expect("GLOBAL_CALLBACKS already set.");

        Self {
            callbacks,
            indi_manager: Arc::new(Mutex::new(IndiManager::new(cc))),
            flats_manager: Default::default(),
            settings_manager: Arc::new(Mutex::new(SettingsManager::new())),
            capture_manager: Arc::new(Mutex::new(CaptureManager::new()))
        }
    }
}

impl eframe::App for App {
    #[tracing::instrument(skip_all)]
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        while let Ok(cb) = self.callbacks.try_recv() {
            cb(ctx, frame);
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                if ui.button("Organize windows").clicked() {
                    ui.ctx().memory_mut(|mem| mem.reset_areas());
                }
            });
        });

        let mut indi_manager = self.indi_manager.lock();
        let mut capture_manager = self.capture_manager.lock();
        let mut settings_manager = self.settings_manager.lock();
        let mut flats_manager = self.flats_manager.lock();

        egui::SidePanel::left("left").show(ctx, |ui| {
            ui.add(indi_manager.deref_mut());
            ui.separator();
            ui.add(flats_manager.deref_mut());
            ui.separator();
            ui.add(settings_manager.deref_mut());
            ui.separator();
            ui.add(capture_manager.deref_mut());
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            indi_manager.windows(ui);
            flats_manager.windows(ui);
            settings_manager.windows(ui);
            capture_manager.windows(ui);
            
        });
    }
}
