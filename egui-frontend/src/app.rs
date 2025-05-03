use crate::flats::FlatWidget;
// use crate::counts::CountIndex;
use crate::indi::IndiManager;
use crate::settings::SettingsWidget;
use crate::sync_task::SyncTask;
use std::boxed::Box;
use std::sync::{mpsc, OnceLock};
static GLOBAL_CALLBACKS: OnceLock<
    std::sync::mpsc::Sender<Box<dyn FnOnce(&egui::Context, &mut eframe::Frame) + Sync + Send>>,
> = OnceLock::new();

pub struct App {
    indi_manager: IndiManager,
    // task_ids: CountIndex,
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
        // let this: Self = if let Some(storage) = cc.storage {
        //     eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        // } else {
        //     Default::default()
        // };

        let (tx, callbacks) = mpsc::channel();
        GLOBAL_CALLBACKS
            .set(tx)
            .expect("GLOBAL_CALLBACKS already set.");

        Self {
            // task_ids: CountIndex::new(cc.egui_ctx.clone()),
            callbacks,
            indi_manager: IndiManager::new(cc),
            flats_manager: SyncTask::new(Default::default(), cc.egui_ctx.clone()),
            settings_manager: SyncTask::new(Default::default(), cc.egui_ctx.clone()),
        }
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    // fn save(&mut self, storage: &mut dyn eframe::Storage) {
    //     self.indi_manager.save(storage);
    // }

    #[tracing::instrument(skip_all)]
    /// Called each time the UI needs repainting, which may be many times per second.
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

        egui::SidePanel::left("left").show(ctx, |ui| {
            ui.add(&mut self.indi_manager);
            ui.separator();
            ui.add(&mut self.flats_manager);
            ui.separator();
            // ui.add(&mut self.task_ids);
            // ui.separator();
            ui.add(&mut self.settings_manager);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // self.task_ids.windows(ui);
            self.indi_manager.windows(ui);
            self.flats_manager.windows(ui);
            self.settings_manager.windows(ui);
        });
    }
}
