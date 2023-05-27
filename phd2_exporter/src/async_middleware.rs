use std::pin::Pin;

use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWriteExt};

pub trait WithAsyncMiddleware<T: tokio::io::AsyncRead + tokio::io::AsyncWrite> {
    fn middleware<F>(self, func: F) -> AsyncMiddleware<T, F>
    where
        F: Fn(&[u8]);
}

impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite> WithAsyncMiddleware<T> for T {
    fn middleware<F>(self, func: F) -> AsyncMiddleware<T, F>
    where
        F: Fn(&[u8]),
    {
        AsyncMiddleware { read: self, func }
    }
}

// impl<T, F> AsyncMiddleware<T, F>
// where
//     T: tokio::io::AsyncRead + tokio::io::AsyncWrite,
//     F: Fn(&[u8]),
// {
//     pub fn new(read: T, func: F) -> Self {
//         AsyncMiddleware { read: read, func }
//     }
// }

#[pin_project]
pub struct AsyncMiddleware<T, F>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite,
    F: Fn(&[u8]),
{
    #[pin]
    read: T,
    func: F,
}

impl<T, F> tokio::io::AsyncRead for AsyncMiddleware<T, F>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + std::marker::Unpin,
    F: Fn(&[u8]),
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let project = self.project();
        let read = project.read;
        let poll = tokio::io::AsyncRead::poll_read(read, cx, buf);

        if let std::task::Poll::Ready(Ok(_)) = poll {
            (project.func)(buf.filled());
        }
        poll
    }
}
impl<T, F> tokio::io::AsyncWrite for AsyncMiddleware<T, F>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + std::marker::Unpin,
    F: Fn(&[u8]),
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        tokio::io::AsyncWrite::poll_write(self.project().read, cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        tokio::io::AsyncWrite::poll_flush(self.project().read, cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        tokio::io::AsyncWrite::poll_shutdown(self.project().read, cx)
    }
}
