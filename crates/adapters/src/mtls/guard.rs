use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::sync::CancellationToken;

pub struct RevocableIo<S> {
    io: S,
    revoked: Pin<Box<dyn Future<Output = ()> + Send>>,
}
impl<S> RevocableIo<S> {
    pub(super) fn new(io: S, token: CancellationToken) -> Self {
        Self {
            io,
            revoked: Box::pin(token.cancelled_owned()),
        }
    }
    fn check(&mut self, context: &mut Context<'_>) -> io::Result<()> {
        if self.revoked.as_mut().poll(context).is_ready() {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "TLS trust changed",
            ));
        }
        Ok(())
    }
}
impl<S: AsyncRead + Unpin> AsyncRead for RevocableIo<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        this.check(context)?;
        Pin::new(&mut this.io).poll_read(context, buffer)
    }
}
impl<S: AsyncWrite + Unpin> AsyncWrite for RevocableIo<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.check(context)?;
        Pin::new(&mut this.io).poll_write(context, bytes)
    }
    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        this.check(context)?;
        Pin::new(&mut this.io).poll_flush(context)
    }
    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_shutdown(context)
    }
}
