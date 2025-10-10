use std::{sync::Arc, time::Duration};

use egui::{DragValue, Window};
use futures::{SinkExt, StreamExt};

use twinkle_api::{capture::{CaptureConfig, CaptureProgress, CaptureRequest, ExposureParameterization, MessageToClient}, FromWebsocketMessage, Message};
use twinkle_client::{sleep, task::{spawn, Abortable, IsRunning, Status, TaskStatusError}};

use crate::{agent::{Agent, AgentLock}, get_http_base};

struct State {
    exposure: f64,
    exposure_param: Option<ExposureParameterization>,
    status: Status<Result<CaptureProgress, TaskStatusError>>,
}

impl Default for State {
    fn default() -> Self {
        State {
            exposure: 0.,
            exposure_param: None,
            status: Status::Pending,
        }
    }
}

pub struct CaptureManager {
    agent: Agent<State>,
}

impl CaptureManager {
    pub fn new() -> Self {
        CaptureManager {
            agent: Default::default(),
            
        }
    }

    pub fn windows(&mut self, ui: &mut egui::Ui) {
        if self.agent.running() {
            let mut open = true;
            Window::new("Capture")
                .open(&mut open)
                .resizable(true)
                .scroll([false, false])
                .show(ui.ctx(), |ui| ui.add(&mut self.agent));
            if !open {
                self.agent.abort();
            }
        }
    }
}

impl egui::Widget for &mut CaptureManager {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| {
            if ui.button("Capture").clicked() {
                self.agent
                    .spawn(ui.ctx().clone(), Default::default(), |state| task(state));
            }
        })
        .response
    }
}

impl egui::Widget for &mut State {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| {
            match &self.exposure_param {
                Some(ep) => {
                    ui.add(DragValue::new(&mut self.exposure).speed(ep.step.as_secs_f32()).range(ep.min.as_secs_f32()..=ep.max.as_secs_f32()));
                },
                None => {
                    ui.spinner();
                }
            }
            match &self.status {
                Status::Pending => ui.label("Pending"),
                Status::Running(Ok(state)) => {
                    ui.add(egui::widgets::ProgressBar::new(state.progress as f32))
                },
                Status::Running(Err(e)) => {
                    ui.label(format!("Error: {:?}", e))
                }
                Status::Completed => ui.label("Complete"),
                Status::Aborted => ui.label("Aborted"),
            };
            ui.horizontal(|ui| {
                if ui.button("Start").clicked() {
                    let params = CaptureRequest::Start(CaptureConfig { exposure: Duration::from_secs_f64(self.exposure)});
                    spawn((), |_| async move {
                        let _ = reqwest::Client::new().post(post_url()).json(&params).send().await;
                    })
                    .abort_on_drop(false);
                }
                if ui.button("Stop").clicked() {
                    let params = CaptureRequest::Stop;
                    spawn((), |_| async move {
                        let _ = reqwest::Client::new().post(post_url()).json(&params).send().await;
                    })
                    .abort_on_drop(false);
                }
                        
            });
        }).response
    }
}


fn get_websocket_url() -> String {
    format!("{}capture", crate::get_websocket_base())
}

fn post_url() -> String {
    format!("{}capture", get_http_base())
}

async fn task(state: Arc<AgentLock<State>>) {
    loop {
        let websocket_url = get_websocket_url();
        let websocket = match tokio_tungstenite_wasm::connect(websocket_url).await {
            Ok(websocket) => websocket,
            Err(e) => {
                tracing::error!("Failed to connect to {}: {:?}", get_websocket_url(), e);
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        let (mut w, mut r) = websocket.split();
        let (_tx, rx) = tokio::sync::oneshot::channel::<()>();
        let close_task = spawn((), move |_| async move {
            if let Err(_) = rx.await {
                if let Err(e) = w.send(Message::Close(None)).await {
                    tracing::error!("Error sending close: {:?}", e);
                }
            }
        })
        .abort_on_drop(false);

        while let Some(Ok(message)) = r.next().await {
            let msg = MessageToClient::from_message(message);
            match msg {
                Ok(MessageToClient::ExposureParameterization(exposure_parameterization)) => {
                    state.write().exposure_param = Some(exposure_parameterization);
                },
                Ok(MessageToClient::Progress(status)) => {
                    state.write().status = status;
                },
                Err(e) => {
                    tracing::error!("Error processing message from server: {:?}", e);
                    break;
                }
            }
        }

        close_task.abort();
    }
}