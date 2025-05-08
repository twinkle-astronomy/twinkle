
use parking_lot::Mutex;

use crate::flats::FlatWidget;
use crate::indi::IndiManager;
use crate::settings::SettingsWidget;
use crate::sync_task::SyncTask;
use std::boxed::Box;
use std::ops::DerefMut;
use std::sync::{mpsc, Arc, OnceLock};
static GLOBAL_CALLBACKS: OnceLock<
    std::sync::mpsc::Sender<Box<dyn FnOnce(&egui::Context, &mut eframe::Frame) + Sync + Send>>,
> = OnceLock::new();

pub struct App {
    indi_manager: Arc<Mutex<IndiManager>>,
    flats_manager: SyncTask<FlatWidget>,
    settings_manager: SyncTask<SettingsWidget>,

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

        let indi_manager = Arc::new(Mutex::new(IndiManager::new(cc)));

        Self {
            // task_ids: CountIndex::new(cc.egui_ctx.clone()),
            callbacks,
            indi_manager: indi_manager,
            flats_manager: SyncTask::new(Default::default(), cc.egui_ctx.clone()),
            settings_manager: SyncTask::new(Default::default(), cc.egui_ctx.clone()),
        }
    }
}

impl eframe::App for App {
    #[tracing::instrument(skip_all)]
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        tracing::info!("update!");

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

        egui::SidePanel::left("left").show(ctx, |ui| {
            ui.add(indi_manager.deref_mut());
            ui.separator();
            ui.add(&mut self.flats_manager);
            ui.separator();
            ui.add(&mut self.settings_manager);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            indi_manager.windows(ui);
            self.flats_manager.windows(ui);
            self.settings_manager.windows(ui);
        });
    }
}
