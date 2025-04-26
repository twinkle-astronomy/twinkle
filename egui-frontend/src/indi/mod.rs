use agent::State;
use eframe::glow;
use egui::Window;
use futures::executor::block_on;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::Mutex;
use twinkle_client::task::Task;

use crate::Agent;

pub mod agent;
pub mod views;
pub mod widgets;

pub struct IndiManager {
    server_addr: String,

    agents: BTreeMap<String, Agent<Arc<Mutex<State>>>>,
    glow: Option<std::sync::Arc<glow::Context>>,
}
impl IndiManager {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let server_addr = cc
            .storage
            .map(|storage| eframe::get_value(storage, "twinkle::indi_manager"))
            .flatten()
            .unwrap_or("indi:7624".to_string());
        IndiManager {
            server_addr,
            agents: Default::default(),
            glow: cc.gl.clone(),
        }
    }

    pub fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "twinkle::indi_manager", &self.server_addr);
    }

    pub fn windows(&mut self, ui: &mut egui::Ui) {
        self.agents.retain(|id, agent| {
            if !block_on(async {
                let status = agent.status().read().await;
                status.running() || status.pending() }) {
                return false;
            }

            let mut open = true;
            {
                Window::new(id.to_string())
                .open(&mut open)
                .resizable(true)
                .scroll([true, false])
                .show(ui.ctx(), |ui| {
                    ui.add(&mut *agent);
                });
            }
            open
        });
    }
}

impl egui::Widget for &mut IndiManager {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {

        ui.vertical(|ui| {
            ui.text_edit_singleline(&mut self.server_addr);

            if ui.button("Connect").clicked() {
                self.agents
                    .entry(format!("indi -> {}", self.server_addr.clone()))
                    .or_insert_with(|| {
                        crate::indi::agent::new(
                            self.server_addr.clone(),
                            ui.ctx().clone(),
                            self.glow.clone(),
                        )
                    });
            }
        })
        .response
    }
}
