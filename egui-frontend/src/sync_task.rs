use std::{
    future::Future,
    ops::{Deref, DerefMut},
};

use egui::{Id, WidgetText, Window};
use twinkle_client::{
    task::{Abortable, AsyncTask, IsRunning},
    MaybeSend,
};

pub struct Sender<T> {
    tx: tokio::sync::mpsc::UnboundedSender<T>,
    ctx: egui::Context,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender {
            tx: self.tx.clone(),
            ctx: self.ctx.clone(),
        }
    }
}

impl<T> Sender<T> {
    pub fn send(&self, msg: T) -> Result<(), tokio::sync::mpsc::error::SendError<T>> {
        self.tx.send(msg)?;
        self.ctx.request_repaint();
        Ok(())
    }
}

pub trait SyncAble {
    type MessageFromTask;
    type MessageToTask;

    fn reset(
        &mut self,
        _tx: tokio::sync::mpsc::UnboundedSender<Self::MessageToTask>,
        _from_task_tx: Sender<Self::MessageFromTask>,
    ) {
    }

    fn update(&mut self, msg: Self::MessageFromTask);
    fn window_name(&self) -> impl Into<WidgetText>;
    fn window_id(&self) -> Id {
        let text: WidgetText = self.window_name().into();
        text.text().to_string().into()
    }
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        tx: tokio::sync::mpsc::UnboundedSender<Self::MessageToTask>,
    ) -> egui::Response;
}
pub struct SyncTask<T: SyncAble> {
    ctx: egui::Context,
    task: AsyncTask<(), ()>,
    value: T,
    from_task_rx: tokio::sync::mpsc::UnboundedReceiver<T::MessageFromTask>,
    to_task_tx: tokio::sync::mpsc::UnboundedSender<T::MessageToTask>,
}

impl<T> SyncTask<T>
where
    T: SyncAble,
{
    pub fn new(value: T, ctx: egui::Context) -> Self {
        let (_, from_task_rx) = tokio::sync::mpsc::unbounded_channel();
        let (to_task_tx, _) = tokio::sync::mpsc::unbounded_channel();
        SyncTask {
            ctx,
            task: Default::default(),
            value,
            from_task_rx,
            to_task_tx,
        }
    }

    pub fn windows(&mut self, ui: &mut egui::Ui) -> bool {
        self.sync_value();
        if self.running() {
            let mut open = true;
            Window::new(self.window_name())
                .open(&mut open)
                .id(self.window_id())
                .resizable(true)
                .scroll([false, false])
                .show(ui.ctx(), |ui| self.value.ui(ui, self.to_task_tx.clone()));
            if !open {
                self.abort();
            }
            return open;
        }
        return false;
    }
}

impl<T: SyncAble> Deref for SyncTask<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: SyncAble> DerefMut for SyncTask<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: SyncAble> SyncTask<T> {
    pub fn spawn<F, U>(&mut self, func: F)
    where
        F: FnOnce(
            Sender<T::MessageFromTask>,
            tokio::sync::mpsc::UnboundedReceiver<T::MessageToTask>,
        ) -> U,
        U: Future<Output = ()> + MaybeSend + 'static,
    {
        let (from_task_tx, from_task_rx) = tokio::sync::mpsc::unbounded_channel();
        self.from_task_rx = from_task_rx;

        let (to_task_tx, to_task_rx) = tokio::sync::mpsc::unbounded_channel();
        self.to_task_tx = to_task_tx;

        self.value.reset(
            self.to_task_tx.clone(),
            Sender {
                tx: from_task_tx.clone(),
                ctx: self.ctx.clone(),
            },
        );
        self.task.spawn((), |_| {
            func(
                Sender {
                    tx: from_task_tx,
                    ctx: self.ctx.clone(),
                },
                to_task_rx,
            )
        });
    }

    pub fn abort(&self) {
        self.task.abort();
    }

    pub fn running(&self) -> bool {
        self.task.running()
    }

    pub fn sync_value(&mut self) {
        if self.from_task_rx.len() > 100 {
            tracing::error!("from_task_rx.len() > 100: {}", self.from_task_rx.len())
        }
        while let Ok(settings) = self.from_task_rx.try_recv() {
            self.value.update(settings)
        }
    }
}
