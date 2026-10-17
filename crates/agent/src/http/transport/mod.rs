mod stream;
use axum_server::accept::Accept;
use hmail_cleaner_adapters::mtls::{RevocableIo, ServerTls};
use std::{future::Future, io, pin::Pin, sync::Arc, time::Duration};
use stream::Connection;
use tokio::{net::TcpStream, sync::Semaphore};
use tokio_rustls::{server::TlsStream, TlsAcceptor};
const MAX_CONNECTIONS: usize = 16;
const MAX_HANDSHAKES: usize = 4;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub(crate) struct Acceptor {
    tls: ServerTls,
    connections: Arc<Semaphore>,
    handshakes: Arc<Semaphore>,
}
impl Acceptor {
    pub fn new(tls: ServerTls) -> Self {
        Self {
            tls,
            connections: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            handshakes: Arc::new(Semaphore::new(MAX_HANDSHAKES)),
        }
    }
}
impl<S: Send + 'static> Accept<TcpStream, S> for Acceptor {
    type Stream = Connection<RevocableIo<TlsStream<TcpStream>>>;
    type Service = S;
    type Future = Pin<Box<dyn Future<Output = io::Result<(Self::Stream, S)>> + Send>>;
    fn accept(&self, stream: TcpStream, service: S) -> Self::Future {
        let (config, lease) = self.tls.snapshot();
        let connection = self.connections.clone().try_acquire_owned();
        let handshake = self.handshakes.clone().try_acquire_owned();
        Box::pin(async move {
            let capacity = || io::Error::new(io::ErrorKind::WouldBlock, "agent connection limit");
            let permit = connection.map_err(|_| capacity())?;
            let handshake = handshake.map_err(|_| capacity())?;
            stream.set_nodelay(true)?;
            let acceptor = TlsAcceptor::from(config);
            let tls = tokio::select! {
                biased;
                _ = lease.cancelled() => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "TLS trust changed")),
                result = tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream)) => result.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TLS handshake timeout"))??,
            };
            drop(handshake);
            Ok((
                Connection {
                    io: lease.guard(tls),
                    _permit: permit,
                },
                service,
            ))
        })
    }
}

const HEADER_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) fn server(
    listener: std::net::TcpListener,
    tls: ServerTls,
    handle: axum_server::Handle<std::net::SocketAddr>,
) -> io::Result<axum_server::Server<std::net::SocketAddr, Acceptor>> {
    listener.set_nonblocking(true)?;
    let mut server = axum_server::from_tcp(listener)?
        .acceptor(Acceptor::new(tls))
        .handle(handle);
    server
        .http_builder()
        .http1()
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(HEADER_TIMEOUT);
    Ok(server)
}
#[cfg(test)]
mod tests;
