use std::pin::Pin;

use crate::{
    client::{
        sink::SinkStringWrapper, stream::StringCommandStream, AsyncClientConnection, Connectable,
    },
    serialization::{self, DeError},
};
use axum::extract::ws::WebSocket;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, Stream, StreamExt,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

impl AsyncClientConnection for WebSocket {
    type Writer =
        SinkStringWrapper<WebSocketCommandWriter<SplitSink<WebSocket, axum::extract::ws::Message>>>;
    type Reader = StringCommandStream<
        Result<String, DeError>,
        WebSocketCommandReader<SplitStream<WebSocket>>,
    >;

    fn to_indi(self) -> (Self::Writer, Self::Reader) {
        let (writer, reader) = self.split();

        (
            WebSocketCommandWriter { writer }.into(),
            WebSocketCommandReader { reader }.into(),
        )
    }
}

pub struct WebSocketCommandWriter<S> {
    writer: S,
}

impl<S> Sink<String> for WebSocketCommandWriter<S>
where
    S: Sink<axum::extract::ws::Message> + std::marker::Unpin,
    serialization::DeError: From<<S as futures::Sink<axum::extract::ws::Message>>::Error>,
{
    type Error = crate::DeError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match Pin::new(&mut self.writer).poll_ready(cx) {
            std::task::Poll::Ready(Ok(r)) => std::task::Poll::Ready(Ok(r)),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(e.into())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: String) -> Result<(), Self::Error> {
        Ok(Pin::new(&mut self.writer).start_send(axum::extract::ws::Message::Text(item))?)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match Pin::new(&mut self.writer).poll_flush(cx) {
            std::task::Poll::Ready(Ok(r)) => std::task::Poll::Ready(Ok(r)),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(e.into())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match Pin::new(&mut self.writer).poll_close(cx) {
            std::task::Poll::Ready(Ok(r)) => std::task::Poll::Ready(Ok(r)),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(e.into())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

pub struct WebSocketCommandReader<S: Stream<Item = Result<axum::extract::ws::Message, axum::Error>>>
{
    reader: S,
}

impl<S: Stream<Item = Result<axum::extract::ws::Message, axum::Error>> + Send + Unpin> Stream
    for WebSocketCommandReader<S>
{
    type Item = Result<String, DeError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // Poll the inner stream
        match Pin::new(&mut self.reader).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(msg))) => std::task::Poll::Ready(match msg {
                axum::extract::ws::Message::Text(text) => Some(Ok(text)),
                axum::extract::ws::Message::Ping(p) => Some(Err(DeError::UnexpectedAxumMessage(
                    axum::extract::ws::Message::Ping(p),
                ))),
                axum::extract::ws::Message::Pong(p) => Some(Err(DeError::UnexpectedAxumMessage(
                    axum::extract::ws::Message::Pong(p),
                ))),
                axum::extract::ws::Message::Close(_) => None,
                axum::extract::ws::Message::Binary(p) => Some(Err(DeError::UnexpectedAxumMessage(
                    axum::extract::ws::Message::Binary(p),
                ))),
            }),
            std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e.into()))),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

pub struct WebSocketStreamCommandWriter<S> {
    writer: S,
}

impl<S> Sink<String> for WebSocketStreamCommandWriter<S>
where
    S: Sink<tokio_tungstenite::tungstenite::Message> + std::marker::Unpin,
    serialization::DeError:
        From<<S as futures::Sink<tokio_tungstenite::tungstenite::Message>>::Error>,
{
    type Error = crate::DeError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match Pin::new(&mut self.writer).poll_ready(cx) {
            std::task::Poll::Ready(Ok(r)) => std::task::Poll::Ready(Ok(r)),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(e.into())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: String) -> Result<(), Self::Error> {
        Ok(Pin::new(&mut self.writer)
            .start_send(tokio_tungstenite::tungstenite::Message::Text(item))?)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match Pin::new(&mut self.writer).poll_flush(cx) {
            std::task::Poll::Ready(Ok(r)) => std::task::Poll::Ready(Ok(r)),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(e.into())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match Pin::new(&mut self.writer).poll_close(cx) {
            std::task::Poll::Ready(Ok(r)) => std::task::Poll::Ready(Ok(r)),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(e.into())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> AsyncClientConnection
    for WebSocketStream<T>
{
    type Writer = SinkStringWrapper<
        WebSocketStreamCommandWriter<
            SplitSink<WebSocketStream<T>, tokio_tungstenite::tungstenite::Message>,
        >,
    >;
    type Reader = StringCommandStream<
        Result<String, DeError>,
        WebSocketStreamCommandReader<SplitStream<WebSocketStream<T>>>,
    >;

    fn to_indi(self) -> (Self::Writer, Self::Reader) {
        let (writer, reader) = self.split();

        (
            WebSocketStreamCommandWriter { writer }.into(),
            WebSocketStreamCommandReader { reader }.into(),
        )
    }
}

impl Connectable for WebSocketStream<MaybeTlsStream<TcpStream>> {
    type ConnectionError = tokio_tungstenite::tungstenite::Error;

    fn connect(
        addr: String,
    ) -> impl std::future::Future<Output = Result<Self, Self::ConnectionError>> + twinkle_client::MaybeSend
    {
        async {
            let (stream, _) = tokio_tungstenite::connect_async(addr).await?;
            Ok(stream)
        }
    }
}

pub struct WebSocketStreamCommandReader<
    S: Stream<
        Item = Result<
            tokio_tungstenite::tungstenite::Message,
            tokio_tungstenite::tungstenite::Error,
        >,
    >,
> {
    reader: S,
}

impl<
        S: Stream<
                Item = Result<
                    tokio_tungstenite::tungstenite::Message,
                    tokio_tungstenite::tungstenite::Error,
                >,
            > + Send
            + Unpin,
    > Stream for WebSocketStreamCommandReader<S>
{
    type Item = Result<String, DeError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match Pin::new(&mut self.reader).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(msg))) => std::task::Poll::Ready(match msg {
                tokio_tungstenite::tungstenite::Message::Text(text) => Some(Ok(text)),
                tokio_tungstenite::tungstenite::Message::Ping(p) => {
                    Some(Err(DeError::UnexpectedTungsteniteMessage(
                        tokio_tungstenite::tungstenite::Message::Ping(p),
                    )))
                }
                tokio_tungstenite::tungstenite::Message::Pong(p) => {
                    Some(Err(DeError::UnexpectedTungsteniteMessage(
                        tokio_tungstenite::tungstenite::Message::Pong(p),
                    )))
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => None,
                tokio_tungstenite::tungstenite::Message::Binary(p) => {
                    Some(Err(DeError::UnexpectedTungsteniteMessage(
                        tokio_tungstenite::tungstenite::Message::Binary(p),
                    )))
                }
                tokio_tungstenite::tungstenite::Message::Frame(frame) => {
                    Some(Err(DeError::UnexpectedTungsteniteMessage(
                        tokio_tungstenite::tungstenite::Message::Frame(frame),
                    )))
                }
            }),
            std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e.into()))),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
