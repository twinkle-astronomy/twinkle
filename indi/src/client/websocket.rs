use crate::{
    client::{AsyncClientConnection, AsyncWriteConnection},
    serialization::{self, Command, DeError},
};
use axum::extract::ws::WebSocket;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream, StreamExt,
};
use tokio::io::{AsyncRead, AsyncWrite};

use super::AsyncReadConnection;

use tokio_tungstenite::WebSocketStream;

impl AsyncClientConnection for WebSocket {
    type Writer = WebSocketCommandWriter<SplitSink<WebSocket, axum::extract::ws::Message>>;
    type Reader = WebSocketCommandReader<SplitStream<WebSocket>>;

    fn to_indi(self) -> (Self::Writer, Self::Reader) {
        let (writer, reader) = self.split();
        (
            WebSocketCommandWriter { writer },
            WebSocketCommandReader { reader },
        )
    }
}

pub struct WebSocketCommandWriter<S> {
    writer: S,
}

impl<S: Sink<axum::extract::ws::Message> + Send + Unpin> AsyncWriteConnection
    for WebSocketCommandWriter<S>
where
    serialization::DeError: From<<S as futures::Sink<axum::extract::ws::Message>>::Error>,
{
    async fn write(&mut self, cmd: Command) -> Result<(), crate::DeError> {
        let msg = quick_xml::se::to_string(&cmd)?;
        self.writer
            .send(axum::extract::ws::Message::Text(msg))
            .await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), crate::DeError> {
        Ok(self.writer.close().await?)
    }
}

pub struct WebSocketCommandReader<S: Stream<Item = Result<axum::extract::ws::Message, axum::Error>>>
{
    reader: S,
}

impl<S: Stream<Item = Result<axum::extract::ws::Message, axum::Error>> + Send + Unpin>
    AsyncReadConnection for WebSocketCommandReader<S>
{
    async fn read(&mut self) -> Option<Result<Command, DeError>> {
        loop {
            let cmd = match self.reader.next().await {
                Some(Ok(c)) => c,
                Some(Err(e)) => return Some(Err(e.into())),
                None => return None,
            };

            match cmd {
                axum::extract::ws::Message::Text(cmd) => {
                    let deser = match quick_xml::de::from_str(cmd.as_str()) {
                        Ok(cmd) => cmd,
                        Err(e) => return Some(Err(e.into())),
                    };

                    return Some(Ok(deser));
                }
                axum::extract::ws::Message::Ping(p) => {
                    dbg!(p);
                },
                axum::extract::ws::Message::Pong(p) => {
                    dbg!(p);
                },
                axum::extract::ws::Message::Close(_) => {
                    return None;
                },
                axum::extract::ws::Message::Binary(p) => {
                    dbg!(p);
                },
            }
        }
    }
}




pub struct WebSocketStreamCommandWriter<S> {
    writer: S,
}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> AsyncClientConnection for WebSocketStream<T> {
    type Writer = WebSocketStreamCommandWriter<SplitSink<WebSocketStream<T>, tokio_tungstenite::tungstenite::Message>>;
    type Reader = WebSocketStreamCommandReader<SplitStream<WebSocketStream<T>>>;

    fn to_indi(self) -> (Self::Writer, Self::Reader) {
        let (writer, reader) = self.split();

        (WebSocketStreamCommandWriter {writer}, WebSocketStreamCommandReader { reader })
    }
}


impl<S: Sink<tokio_tungstenite::tungstenite::Message> + Send + Unpin> AsyncWriteConnection
    for WebSocketStreamCommandWriter<S>
where
    serialization::DeError: From<<S as futures::Sink<tokio_tungstenite::tungstenite::Message>>::Error>,
{
    async fn write(&mut self, cmd: Command) -> Result<(), crate::DeError> {
        let msg = quick_xml::se::to_string(&cmd)?;
        self.writer
            .send(tokio_tungstenite::tungstenite::Message::Text(msg))
            .await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), crate::DeError> {
        Ok(self.writer.close().await?)
    }
}

pub struct WebSocketStreamCommandReader<S: Stream<Item = Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>>>
{
    reader: S,
}

impl<S: Stream<Item = Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>> + Send + Unpin>
    AsyncReadConnection for WebSocketStreamCommandReader<S>
{
    async fn read(&mut self) -> Option<Result<Command, DeError>> {
        loop {
            let cmd = match self.reader.next().await {
                Some(Ok(c)) => c,
                Some(Err(e)) => return Some(Err(e.into())),
                None => return None,
            };

            match cmd {
                tokio_tungstenite::tungstenite::Message::Text(cmd) => {
                    let deser = match quick_xml::de::from_str(cmd.as_str()) {
                        Ok(cmd) => cmd,
                        Err(e) => return Some(Err(e.into())),
                    };

                    return Some(Ok(deser));
                }
                _ => unimplemented!(),
            }
        }
    }
}
