mod state;
pub use state::AppState;

use crate::{
    config::CenterConfig,
    http::{self},
    storage::{CenterStore, DataDirectory},
};
use axum_server::tls_rustls::RustlsConfig;
use hmail_cleaner_core::cleanup::Cancellation;
use std::{sync::Arc, time::Duration};
const HEADER_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_TICK: Duration = Duration::from_secs(1);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);
struct SessionLifetime(Arc<crate::auth::SessionAuthority>);
impl Drop for SessionLifetime {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}
#[derive(Debug, thiserror::Error)]
#[error("center startup or transport failed")]
pub struct CenterError;
pub async fn run(config: CenterConfig, cancel: Arc<Cancellation>) -> Result<(), CenterError> {
    run_with_build(config, cancel, env!("CARGO_PKG_VERSION")).await
}
pub(crate) async fn run_with_build(
    config: CenterConfig,
    cancel: Arc<Cancellation>,
    build_version: &'static str,
) -> Result<(), CenterError> {
    let lease = DataDirectory::acquire(&config.data_dir).map_err(|_| CenterError)?;
    let store = CenterStore::open(&lease).await.map_err(|_| CenterError)?;
    let state = AppState::new(store, config.origin.clone())
        .await
        .map_err(|_| CenterError)?
        .with_build_version(build_version)
        .map_err(|_| CenterError)?;
    let state = state.with_static_dir(
        http::static_files::StaticDir::open(&config.static_dir).map_err(|_| CenterError)?,
    );
    state
        .enrollment
        .recover_interrupted()
        .await
        .map_err(|_| CenterError)?;
    let listener = std::net::TcpListener::bind(config.listen).map_err(|_| CenterError)?;
    serve(listener, &config, state, cancel).await
}
pub async fn serve(
    listener: std::net::TcpListener,
    config: &CenterConfig,
    state: AppState,
    cancel: Arc<Cancellation>,
) -> Result<(), CenterError> {
    let _session_lifetime = SessionLifetime(state.authority.clone());
    use std::io::Read;
    fn bounded(path: &std::path::Path) -> Result<Vec<u8>, CenterError> {
        let meta = path.symlink_metadata().map_err(|_| CenterError)?;
        if !meta.is_file() || meta.len() > 65536 {
            return Err(CenterError);
        }
        let mut output = Vec::new();
        std::fs::File::open(path)
            .and_then(|f| f.take(65537).read_to_end(&mut output))
            .map_err(|_| CenterError)?;
        if output.len() > 65536 {
            return Err(CenterError);
        }
        Ok(output)
    }
    let certificate = bounded(&config.data_dir.join("https-cert.pem"))?;
    let private = zeroize::Zeroizing::new(bounded(&config.data_dir.join("https-key.pem"))?);
    // Explicit provider; no process-global default provider or AWS-LC dependency.
    let mut cert_reader = std::io::Cursor::new(&certificate);
    let certs = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CenterError)?;
    let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(&*private))
        .map_err(|_| CenterError)?
        .ok_or(CenterError)?;
    let mut tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| CenterError)?
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .map_err(|_| CenterError)?;
    tls.alpn_protocols = vec![b"http/1.1".to_vec()];
    let acceptor = http::server::BoundedTls::new(RustlsConfig::from_config(Arc::new(tls)));
    listener.set_nonblocking(true).map_err(|_| CenterError)?;
    let handle = axum_server::Handle::new();
    let mut server = axum_server::from_tcp(listener)
        .map_err(|_| CenterError)?
        .acceptor(acceptor)
        .http1_only()
        .handle(handle.clone());
    server
        .http_builder()
        .http1()
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(HEADER_TIMEOUT)
        .max_buf_size(32768);
    let authority = state.authority.clone();
    let future = server.serve(http::router(state).into_make_service());
    tokio::pin!(future);
    let mut tick = tokio::time::interval(IDLE_TICK);
    loop {
        tokio::select! {
            result=&mut future=>{return result.map_err(|_|CenterError);}
            _=tick.tick()=>{
                authority.expire();
                if cancel.is_requested() {
                    authority.shutdown();
                    handle.graceful_shutdown(Some(SHUTDOWN_GRACE));
                    return future.await.map_err(|_|CenterError);
                }
            }
        }
    }
}
