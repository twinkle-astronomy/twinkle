use std::sync::Arc;

use bytes::BytesMut;
use eframe::glow;
use egui::{mutex::Mutex, ProgressBar};
use futures::executor::block_on;
use reqwest::IntoUrl;
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio_stream::StreamExt;
use tracing::{debug, error, info, Instrument};
use twinkle_api::indi::api::ImageResponse;
use url::Url;

use crate::task::{self, AsyncTask};

use super::{FitsRender, FitsWidget};

pub struct ImageView {
    sender: Sender<Url>,
    state: Arc<tokio::sync::Mutex<State>>,
}

struct State {
    render: Arc<Mutex<FitsRender>>,
    progress: f32,
}

impl crate::Widget for &ImageView {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        
        let state = block_on(self.state.lock());
        ui.vertical(|ui| {
            
            ui.add(FitsWidget::new(state.render.clone()));
            ui.add(ProgressBar::new(state.progress));
        }).response
    }
}
impl ImageView {
    pub fn new(gl: &glow::Context) -> AsyncTask<(), ImageView> {
        debug!("new ImageView");
        let (sender, rx) = broadcast::channel(1);
        let state = Arc::new(tokio::sync::Mutex::new(State {
            render: Arc::new(Mutex::new(FitsRender::new(gl))),
            progress: 0.0,
        }));
        task::spawn_with_state(ImageView {sender, state}, |state| Self::process_downloads(state.state.clone(), rx))
            .abort_on_drop(true)
        
    }
    pub async fn download_image(&self, url: impl IntoUrl + 'static) -> Result<(), reqwest::Error> {
        let _ = self.sender.send(url.into_url()?);
        Ok(())
    }

    async fn process_downloads(state: Arc<tokio::sync::Mutex<State>>, mut rx: Receiver<Url>) {
        loop {

            let url = match rx.recv().await {
                Ok(url) => url,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    continue;   
                }
                Err(_) => {
                    return;
                }
            };
            {
                state.lock().await.progress = 0.;
            }

            let bytes = async {
                let client = reqwest::Client::new();

                // Use a relative path - reqwest will use the current origin in a WASM context
                let response = client.get(url).send().await.unwrap();

                if !response.status().is_success() {
                    // You might want to handle this differently
                    error!("HTTP error: {}", response.status());
                }
                let total_size = response.content_length().unwrap_or(0);

                // Prepare a buffer for the data
                let mut buffer = BytesMut::new();
                let mut downloaded = 0;

                // Get the response as a byte stream
                let mut stream = response.bytes_stream();

                // Process the stream chunk by chunk
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.unwrap(); // Handle this error appropriately in production
                    downloaded += chunk.len() as u64;
                    buffer.extend_from_slice(&chunk);

                    // Calculate and report progress
                    if total_size > 0 {
                        let progress = (downloaded as f32) / (total_size as f32);
                        state.lock().await.progress = progress;
                    }
                }

                buffer.to_vec()
            }
            .instrument(tracing::info_span!("download_indi_image"))
            .await;

            info!("Got {} bytes downloaded", bytes.len());

            let resp: ImageResponse<'_> = {
                let _span = tracing::info_span!("rmp_serde::from_slice").entered();
                twinkle_api::indi::api::ImageResponse::from_bytes(bytes.as_ref()).unwrap()
            };

            let data = {
                let _span = tracing::info_span!("read_fits").entered();
                resp.image.read_image().unwrap()
            };
            {
                let state = state.lock().await;
                let mut image_view = state.render.lock();
                image_view.set_fits(data);
                image_view.auto_stretch(&resp.stats);
            }
        }
    }
}
