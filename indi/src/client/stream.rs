use std::{future::Future, pin::Pin};

use futures::Stream;

use crate::{Command, DeError};

use super::{AsyncReadConnection, MaybeSend};
use tokio_stream::StreamExt;

pub struct ParsedCommandStream<'a, T, S: Stream<Item = T>>(&'a mut S);

impl<'a, S> From<&'a mut S> for ParsedCommandStream<'a, Result<Command, DeError>, S>
where
    S: Stream<Item = Result<Command, DeError>>,
{
    fn from(stream: &'a mut S) -> Self {
        ParsedCommandStream(stream)
    }
}

impl<T> AsyncReadConnection for T
where
    T: Stream<Item = Result<Command, DeError>> + Unpin + MaybeSend,
    for<'a> &'a mut T: Into<ParsedCommandStream<'a, Result<Command, DeError>, T>>,
{
    fn read(&mut self) -> impl Future<Output = Option<Result<crate::Command, crate::DeError>>> {
        async move { ParsedCommandStream::from(self).0.next().await }
    }
}

pub struct StringCommandStream<T, S: Stream<Item = T>>(S);

impl<S> From<S> for StringCommandStream<Result<String, DeError>, S>
where
    S: Stream<Item = Result<String, DeError>>,
{
    fn from(stream: S) -> Self {
        StringCommandStream(stream)
    }
}

impl<S> Stream for StringCommandStream<Result<String, DeError>, S>
where
    S: Stream<Item = Result<String, DeError>> + Unpin,
{
    type Item = Result<Command, DeError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // Poll the inner stream
        match Pin::new(&mut self.0).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(value))) => {
                // Convert the String to Command
                let result = quick_xml::de::from_str(value.as_str()).map_err(Into::into);
                std::task::Poll::Ready(Some(result))
            }
            std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e))),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
