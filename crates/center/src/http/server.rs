use axum_server::{
    accept::Accept,
    tls_rustls::{RustlsAcceptor, RustlsConfig},
};
use std::{
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    sync::{OwnedSemaphorePermit, Semaphore},
};
const MAX_CONNECTIONS: usize = 64;
const MAX_HANDSHAKES: usize = 4;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
#[derive(Clone)]
pub(crate) struct BoundedTls {
    tls: RustlsAcceptor,
    connections: Arc<Semaphore>,
    handshakes: Arc<Semaphore>,
}
impl BoundedTls {
    pub fn new(config: RustlsConfig) -> Self {
        Self {
            tls: RustlsAcceptor::new(config),
            connections: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            handshakes: Arc::new(Semaphore::new(MAX_HANDSHAKES)),
        }
    }
}
pub(crate) struct Connection<T> {
    io: T,
    _permit: OwnedSemaphorePermit,
}
impl<T: AsyncRead + Unpin> AsyncRead for Connection<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_read(cx, buf)
    }
}
impl<T: AsyncWrite + Unpin> AsyncWrite for Connection<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().io).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_shutdown(cx)
    }
}
impl<S: Send + 'static> Accept<TcpStream, S> for BoundedTls {
    type Stream = Connection<tokio_rustls::server::TlsStream<TcpStream>>;
    type Service = S;
    type Future = Pin<Box<dyn Future<Output = io::Result<(Self::Stream, S)>> + Send>>;
    fn accept(&self, stream: TcpStream, service: S) -> Self::Future {
        let connection = self.connections.clone().try_acquire_owned();
        let handshake = self.handshakes.clone().try_acquire_owned();
        let tls = self.tls.clone();
        Box::pin(async move {
            let permit = connection.map_err(|_| io::Error::from(io::ErrorKind::WouldBlock))?;
            let _handshake = handshake.map_err(|_| io::Error::from(io::ErrorKind::WouldBlock))?;
            let (stream, service) =
                tokio::time::timeout(HANDSHAKE_TIMEOUT, tls.accept(stream, service))
                    .await
                    .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))??;
            Ok((
                Connection {
                    io: stream,
                    _permit: permit,
                },
                service,
            ))
        })
    }
}
