use agent::State;
use eframe::glow;
use egui::Window;
use futures::executor::block_on;
use uuid::Uuid;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::Mutex;
use twinkle_client::task::Task;

use crate::Agent;

pub mod agent;
pub mod views;
pub mod widgets;
pub mod control;

pub struct IndiManager {
    agents: BTreeMap<Uuid, Agent<Arc<Mutex<State>>>>,
    glow: Option<std::sync::Arc<glow::Context>>,
}


impl IndiManager  {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        IndiManager {
            agents: Default::default(),
            glow: cc.gl.clone(),
        }
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
                Window::new("Indi")
                .open(&mut open)
                .id(id.to_string().into())
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
            if ui.button("Connect").clicked() {
                self.agents
                    .insert(Uuid::new_v4(),
                        crate::indi::agent::new(
                            ui.ctx().clone(),
                            self.glow.clone(),
                        )
                );
            }
        })
        .response
    }
}
