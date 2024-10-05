use axum::extract::ws::{Message, WebSocket};
use futures::{stream::{SplitSink, SplitStream}, SinkExt, StreamExt};
use crate::{client::{AsyncClientConnection, AsyncWriteConnection}, serialization::{Command, DeError}};

use super::AsyncReadConnection;

impl AsyncClientConnection for WebSocket {
    type Writer = WebSocketCommandWriter;
    type Reader = WebSocketCommandReader;

    fn to_indi(self) -> (Self::Writer, Self::Reader) {
        let (writer, reader) = self.split();
        (WebSocketCommandWriter {writer }, WebSocketCommandReader {reader })
    }
}

pub struct WebSocketCommandWriter {
    writer: SplitSink<WebSocket, Message>
}

impl AsyncWriteConnection for WebSocketCommandWriter {
    async fn write(&mut self, cmd: Command) -> Result<(), crate::DeError> {
        let msg = quick_xml::se::to_string(&cmd)?;
        self.writer.send(Message::Text(msg)).await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), crate::DeError> {
        Ok(self.writer.close().await?)
    }
}


pub struct WebSocketCommandReader {
    reader: SplitStream<WebSocket>
}

impl AsyncReadConnection for WebSocketCommandReader {
    async fn read(&mut self) -> Option<Result<Command, DeError>> {
        loop {
            let cmd = match self.reader.next().await {
                Some(Ok(c)) => c,
                Some(Err(e)) => {
                    return Some(Err(e.into()))
                },
                None => return None,
            };

            match cmd {
                Message::Text(cmd) => {
                    let deser = match quick_xml::de::from_str(cmd.as_str()) {
                        Ok(cmd) => cmd,
                        Err(e) => return Some(Err(e.into())),
                    };
                    
                    return Some(Ok(deser));
                },
                _ => unimplemented!(),
            }
        }
    }
}