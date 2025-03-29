use crate::{indi::agent::State, Agent};
use egui::Window;
use fitsrs::hdu::header::extension::bintable::A;
use futures::executor::block_on;
use twinkle_api::Count;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{mpsc, Arc, OnceLock},
};
use tokio::sync::Mutex;
use twinkle_client::task::Task;
use std::boxed::Box;

static GLOBAL_CALLBACKS: OnceLock<
    std::sync::mpsc::Sender<Box<dyn FnOnce(&egui::Context, &mut eframe::Frame) + Sync + Send>>,
> = OnceLock::new();

trait TaskWidget: Task<()> {}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    server_addr: String,

    #[serde(skip)]
    agents: BTreeMap<String, Agent<(), Arc<Mutex<State>>>>,

    #[serde(skip)]
    tasks: BTreeSet<String, Box<dyn TaskWidget<AsyncLock = _>>>,

    #[serde(skip)]
    callbacks: std::sync::mpsc::Receiver<
        Box<dyn FnOnce(&egui::Context, &mut eframe::Frame) + Sync + Send>,
    >,
}

impl Default for App {
    fn default() -> Self {
        let (tx, callbacks) = mpsc::channel();
        GLOBAL_CALLBACKS
            .set(tx)
            .expect("GLOBAL_CALLBACKS already set.");
        Self {
            server_addr: "indi:7624".to_string(),
            agents: Default::default(),
            tasks: Default::default(),
            callbacks,
        }
    }
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
        let this: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        this
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    #[tracing::instrument(skip_all)]
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        while let Ok(cb) = self.callbacks.try_recv() {
            cb(ctx, frame);
        }
        self.agents.retain(|id, agent| {
            if !block_on(async {agent.status().read().await.running()}) {
                return false;
            }
            let mut open = true;
            Window::new(id.to_string())
                .open(&mut open)
                .resizable(true)
                .scroll([true, false])
                .show(ctx, |ui| {
                    ui.add(agent);
                });
            open
        });
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.text_edit_singleline(&mut self.server_addr);

            if ui.button("Connect").clicked() {
                self.agents
                    .entry(format!("indi -> {}", self.server_addr.clone()))
                    .or_insert_with(|| {
                        crate::indi::agent::new(
                            self.server_addr.clone(),
                            ctx.clone(),
                            frame.gl().cloned(),
                        )
                    });
            }
        });
    }
}
