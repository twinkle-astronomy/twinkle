use futures::{
    stream::{SplitSink, SplitStream},
    Sink, Stream, StreamExt,
};
use twinkle_client::MaybeSend;

use crate::{
    client::{
        sink::SinkStringWrapper, stream::StringCommandStream, AsyncClientConnection, Connectable,
    },
    serialization, DeError,
};
use std::{future::Future, pin::Pin};
use tokio_tungstenite_wasm::{Error, Message, WebSocketStream};

impl AsyncClientConnection for tokio_tungstenite_wasm::WebSocketStream {
    type Writer = SinkStringWrapper<WebSocketCommandWriter<SplitSink<WebSocketStream, Message>>>;
    type Reader = StringCommandStream<
        Result<String, DeError>,
        WebSocketCommandReader<SplitStream<WebSocketStream>>,
    >;

    fn to_indi(self) -> (Self::Writer, Self::Reader) {
        let (writer, reader) = self.split();

        (
            WebSocketCommandWriter { writer }.into(),
            WebSocketCommandReader { reader }.into(),
        )
    }
}

impl Connectable for tokio_tungstenite_wasm::WebSocketStream {
    type ConnectionError = tokio_tungstenite_wasm::Error;

    fn connect(
        addr: String,
    ) -> impl Future<Output = Result<Self, Self::ConnectionError>> + MaybeSend {
        tokio_tungstenite_wasm::connect(addr)
    }
}

pub struct WebSocketCommandReader<S: Stream<Item = Result<Message, Error>>> {
    reader: S,
}

impl<S: Stream<Item = Result<Message, Error>> + Unpin> Stream for WebSocketCommandReader<S> {
    type Item = Result<String, DeError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match Pin::new(&mut self.reader).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(msg))) => std::task::Poll::Ready(match msg {
                Message::Text(text) => Some(Ok(text)),
                Message::Close(_) => None,
                Message::Binary(p) => Some(Err(DeError::UnexpectedTungsteniteWasmMessage(
                    Message::Binary(p),
                ))),
            }),
            std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e.into()))),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
pub struct WebSocketCommandWriter<S> {
    writer: S,
}

impl<S> Sink<String> for WebSocketCommandWriter<S>
where
    S: Sink<Message> + std::marker::Unpin,
    serialization::DeError: From<<S as futures::Sink<Message>>::Error>,
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
        Ok(Pin::new(&mut self.writer).start_send(Message::Text(item))?)
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
