use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use twinkle_client::task::{AsyncTask, Status, Task};

#[derive(Default)]
pub struct AgentLock<T> {
    lock: egui::mutex::RwLock<T>,
    ctx: egui::Context,
}

impl<T> Widget for &AgentLock<T>
where
    for<'a> &'a mut T: Widget,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut guard = self.lock.write();
        guard.deref_mut().ui(ui)
    }
}

pub struct AgentLockWriteGuard<'a, T> {
    guard: egui::mutex::RwLockWriteGuard<'a, T>,
    ctx: egui::Context,
}

impl<'a, T> Deref for AgentLockWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<'a, T> DerefMut for AgentLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}

impl<'a, T> Drop for AgentLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        tracing::info!("request_repaint");
        self.ctx.request_repaint();
    }
}

#[derive(derive_more::Deref, derive_more::DerefMut, derive_more::From)]
pub struct AgentLockReadGuard<'a, T>(egui::mutex::RwLockReadGuard<'a, T>);

impl<T> AgentLock<T> {
    pub fn new(ctx: egui::Context, value: T) -> Self {
        Self {
            lock: egui::mutex::RwLock::new(value),
            ctx,
        }
    }

    pub fn write<'a>(&'a self) -> AgentLockWriteGuard<'a, T> {
        AgentLockWriteGuard {
            guard: self.lock.write(),
            ctx: self.ctx.clone(),
        }
    }
    pub fn read<'a>(&'a self) -> AgentLockReadGuard<'a, T> {
        self.lock.read().into()
    }
}

#[derive(derive_more::From, derive_more::Deref, derive_more::DerefMut)]
pub struct Agent<S: std::marker::Sync>(twinkle_client::task::AsyncTask<(), Arc<AgentLock<S>>>);

impl<S: Sync> Default for Agent<S> {
    fn default() -> Self {
        Agent(AsyncTask::default())
    }
}

pub trait Widget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response;
}

impl<S: Send + Sync + 'static> egui::Widget for &mut Agent<S>
where
    for<'a> &'a mut S: Widget,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let status = futures::executor::block_on(self.status().read());
        if let Status::Running(state) = status.deref() {
            tracing::info!("create write state lock");
            state.ui(ui)
        } else {
            ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
        }
    }
}
