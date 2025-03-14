use eframe::glow;
use egui::Window;
use twinkle_client::OnDropFutureExt;
use std::collections::BTreeMap;
use crate::{
    indi::agent::IndiAgent,
    task::{Status, Task},
};

pub trait Agent {
    fn show(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame);
    fn on_exit(&mut self, gl: Option<&glow::Context>);
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    server_addr: String,

    #[serde(skip)]
    agents: BTreeMap<String, IndiAgent>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            server_addr: "indi:7624".to_string(),
            agents: Default::default(),
        }
    }
}

impl App {
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

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.agents.retain(|id, agent| {
            if agent.status() != Status::Running {
                agent.on_exit(frame.gl().map(|v| &**v));
                return false;
            }
            let mut open = true;
            Window::new(id.to_string())
                .open(&mut open)
                .resizable(true)
                .scroll([true, false])
                .show(ctx, |ui| {
                    agent.show(ui, frame);
                });
            if !open {
                agent.abort();
            }
            true
        });
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.text_edit_singleline(&mut self.server_addr);

            if ui.button("Connect").clicked() {
                self.agents.insert(
                    format!("indi -> {}", self.server_addr.clone()),
                    crate::indi::agent::new(
                        self.server_addr.clone(),
                        ctx.clone(),
                        frame.gl().cloned(),
                    ),
                );
            }
        });
    }
}
