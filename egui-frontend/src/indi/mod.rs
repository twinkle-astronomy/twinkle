use agent::State;
use eframe::glow;
use uuid::Uuid;
use std::collections::BTreeMap;

use crate::sync_task::SyncTask;

pub mod agent;
pub mod views;
pub mod widgets;
pub mod control;

pub struct IndiManager {
    agents: BTreeMap<Uuid, SyncTask<State>>,
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
        self.agents.retain(|_id, agent| {
            agent.windows(ui)
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
                            ui.ctx().clone(), self.glow.clone(),
                        )
                );
            }
        })
        .response
    }
}
