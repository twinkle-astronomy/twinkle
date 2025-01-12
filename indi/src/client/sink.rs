use std::pin::Pin;

use futures::Sink;

use crate::{Command, DeError};
use futures::SinkExt;

use super::{AsyncWriteConnection, MaybeSend};

pub struct SinkCommandWrapper<'a, S>
where
    S: Sink<Command> + Unpin,
    S::Error: Into<crate::DeError>,
{
    inner: &'a mut S,
}

impl<'a, S> From<&'a mut S> for SinkCommandWrapper<'a, S>
where
    S: Sink<Command, Error = DeError> + Unpin,
{
    fn from(value: &'a mut S) -> Self {
        SinkCommandWrapper { inner: value }
    }
}

impl<T> AsyncWriteConnection for T
where
    T: Sink<Command, Error = DeError> + Unpin + MaybeSend,
    for<'a> &'a mut T: Into<SinkCommandWrapper<'a, T>>,
{
    fn shutdown(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), crate::DeError>> + MaybeSend {
        async move { SinkCommandWrapper::from(self).inner.close().await }
    }

    fn write(
        &mut self,
        cmd: Command,
    ) -> impl std::future::Future<Output = Result<(), crate::DeError>> + MaybeSend {
        async move {
            SinkCommandWrapper::from(self).inner.send(cmd).await?;
            Ok(())
        }
    }
}

pub struct SinkStringWrapper<S>
where
    S: Sink<String> + Unpin,
    S::Error: Into<crate::DeError>,
{
    inner: S,
}

impl<S> From<S> for SinkStringWrapper<S>
where
    S: Sink<String, Error = DeError> + Unpin,
{
    fn from(value: S) -> Self {
        SinkStringWrapper { inner: value }
    }
}

impl<S> Sink<Command> for SinkStringWrapper<S>
where
    S: Sink<String, Error = DeError> + Unpin,
{
    type Error = crate::DeError;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_ready(cx)
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: Command) -> Result<(), Self::Error> {
        Pin::new(&mut self.inner).start_send(quick_xml::se::to_string(&item)?)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner).poll_close(cx)
    }
}
