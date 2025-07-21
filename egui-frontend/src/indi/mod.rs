use agent::State;
use eframe::glow;
use egui::{Widget, Window};
use std::collections::BTreeMap;
use twinkle_client::task::{Abortable, IsRunning};
use uuid::Uuid;

use crate::agent::Agent;

pub mod agent;
pub mod control;
pub mod views;
pub mod widgets;

pub struct IndiManager {
    agents: BTreeMap<Uuid, (String, Agent<State>)>,
    glow: Option<std::sync::Arc<glow::Context>>,
}

impl IndiManager {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        IndiManager {
            agents: Default::default(),
            glow: cc.gl.clone(),
        }
    }

    pub fn windows(&mut self, ui: &mut egui::Ui) {
        self.agents.retain(|id, (title, agent)| {
            if agent.running() {
                let mut open = true;
                Window::new(title.as_str())
                    .open(&mut open)
                    .id(id.to_string().into())
                    .resizable(true)
                    .scroll([false, false])
                    .show(ui.ctx(), |ui| agent.ui(ui));
                if !open {
                    agent.abort();
                }
                return open;
            }
            return false;
        });
    }
}

impl egui::Widget for &mut IndiManager {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| {
            ui.label("Indi");
            if ui.button("Devices").clicked() {
                self.agents
                    .insert(Uuid::new_v4(), ("Indi - devices".to_string(), crate::indi::agent::new(false, ui.ctx().clone(), self.glow.clone())));
            }
            if ui.button("Images").clicked() {
                self.agents
                    .insert(Uuid::new_v4(), ("Indi - images".to_string(), crate::indi::agent::new(true, ui.ctx().clone(), self.glow.clone())));
            }
        })
        .response
    }
}
