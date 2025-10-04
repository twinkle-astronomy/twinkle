use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures::{SinkExt, Stream, StreamExt};
use twinkle_api::ToWebsocketMessage;

pub struct WebsocketHandler {
    socket: WebSocket,
    sender: Option<tokio::sync::mpsc::Sender<Message>>,
}

impl From<WebSocket> for WebsocketHandler {
    fn from(value: WebSocket) -> Self {
        WebsocketHandler {
            socket: value,
            sender: None,
        }
    }
}

impl WebsocketHandler {
    pub fn set_sender(
        &mut self,
        sender: tokio::sync::mpsc::Sender<Message>,
    ) -> Option<tokio::sync::mpsc::Sender<Message>> {
        self.sender.replace(sender)
    }

    pub async fn handle_websocket_stream<S, T>(mut self, mut stream: S)
    where
        S: Stream<Item = T> + Unpin,
        T: ToWebsocketMessage,
    {
        let (ws_send, mut ws_recv) = tokio::sync::broadcast::channel(1024);
        let (mut w, mut r) = self.socket.split();

        let reader_future = {
            let ws_send = ws_send.clone();
            let sender = self.sender.take();
            async move {
                while let Some(msg) = r.next().await {
                    match msg {
                        Ok(Message::Close(msg)) => {
                            if let Err(e) = ws_send.send(Message::Close(msg)) {
                                tracing::error!("Got error sending close: {:?}", e);
                            }
                        }
                        Ok(Message::Ping(p)) => {
                            if let Err(e) = ws_send.send(Message::Pong(p)) {
                                tracing::error!("Got error sending pong: {:?}", e);
                            }
                        }
                        Ok(msg) => {
                            if let Some(sender) = &sender {
                                if let Err(e) = sender.send(msg).await {
                                    tracing::error!("Error processing incoming message: {:?}", e);
                                    if let Err(e) = ws_send.send(Message::Close(Some(CloseFrame {
                                        code: axum::extract::ws::close_code::ERROR,
                                        reason: "Error processing incoming message".into(),
                                    }))) {
                                        tracing::error!("Got error sending close: {:?}", e);
                                    }
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Got error reading websocket: {:?}", e);
                        }
                    }
                }
            }
        };

        let writer_future = {
            async move {
                loop {
                    let msg = ws_recv.recv().await;
                    match msg {
                        Ok(msg) => {
                            let is_close = match &msg {
                                Message::Close(_) => true,
                                _ => false,
                            };

                            if let Err(e) = w.send(msg).await {
                                tracing::error!("Got error sending websocket message: {:?}", e);
                                break;
                            }
                            if is_close {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error streaming items: {:?}", e);
                        }
                    }
                }

                w.close().await.ok();
            }
        };

        let stream_future = {
            let ws_send = ws_send.clone();
            async move {
                while let Some(next) = stream.next().await {
                    if let Err(e) = ws_send.send(next.to_message()) {
                        tracing::error!("Error streaming settings: {:?}", e);
                        break;
                    }
                }

                // Send close message to close the websocket cleanly
                ws_send
                    .send(Message::Close(Some(CloseFrame {
                        code: axum::extract::ws::close_code::NORMAL,
                        reason: "End of data".into(),
                    })))
                    .ok();

                // Wait forever to allow connection management futures
                // to complete their work.
                std::future::pending::<()>().await;
            }
        };

        tokio::select! {
            _ = reader_future => {}
            _ = writer_future => {}
            _ = stream_future => {}
        }
    }
}
