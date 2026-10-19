use crate::{
    config::{validate_build_version, CenterConfig},
    http::health::Health,
    storage::SCHEMA_VERSION,
};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use hyper_util::rt::TokioIo;
use std::{
    io::Read,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio_rustls::{rustls::pki_types::ServerName, TlsConnector};

const HEALTH_DEADLINE: Duration = Duration::from_secs(10);
const MAX_CA_BYTES: u64 = 65536;
const MAX_HEALTH_BYTES: usize = 4096;
#[derive(Debug, thiserror::Error)]
pub(super) enum HealthError {
    #[error("center health configuration is invalid")]
    Configuration,
    #[error("center HTTPS health is unavailable or untrusted")]
    Unavailable,
    #[error("center health response violates the protocol")]
    Protocol,
    #[error("center build does not match the expected build")]
    BuildMismatch,
}
fn tls(config: &CenterConfig) -> Result<rustls::ClientConfig, HealthError> {
    let path = config.data_dir.join("ca-cert.pem");
    let metadata = path
        .symlink_metadata()
        .map_err(|_| HealthError::Configuration)?;
    if !metadata.is_file() || metadata.len() > MAX_CA_BYTES {
        return Err(HealthError::Configuration);
    }
    let mut pem = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_CA_BYTES + 1).read_to_end(&mut pem))
        .map_err(|_| HealthError::Configuration)?;
    if pem.len() as u64 > MAX_CA_BYTES {
        return Err(HealthError::Configuration);
    }
    let mut roots = rustls::RootCertStore::empty();
    for certificate in rustls_pemfile::certs(&mut std::io::Cursor::new(pem)) {
        roots
            .add(certificate.map_err(|_| HealthError::Configuration)?)
            .map_err(|_| HealthError::Configuration)?;
    }
    if roots.is_empty() {
        return Err(HealthError::Configuration);
    }
    Ok(rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| HealthError::Configuration)?
    .with_root_certificates(roots)
    .with_no_client_auth())
}
fn address(listen: SocketAddr) -> SocketAddr {
    let ip = match listen.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    SocketAddr::new(ip, listen.port())
}
async fn request(
    config: &CenterConfig,
    tls: rustls::ClientConfig,
    expected_build: Option<&str>,
) -> Result<Health, HealthError> {
    let socket = tokio::net::TcpStream::connect(address(config.listen))
        .await
        .map_err(|_| HealthError::Unavailable)?;
    let name = ServerName::try_from(config.origin.host().to_owned())
        .map_err(|_| HealthError::Configuration)?;
    let stream = TlsConnector::from(Arc::new(tls))
        .connect(name, socket)
        .await
        .map_err(|_| HealthError::Unavailable)?;
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|_| HealthError::Unavailable)?;
    let operation = async {
        let input = Request::get("/healthz")
            .header(header::HOST, config.origin.authority())
            .header(header::ACCEPT, "application/json")
            .header(header::CONNECTION, "close")
            .body(Body::empty())
            .map_err(|_| HealthError::Configuration)?;
        let response = sender
            .send_request(input)
            .await
            .map_err(|_| HealthError::Unavailable)?;
        if response.status() != StatusCode::OK {
            return Err(HealthError::Unavailable);
        }
        if response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            != Some("application/json")
        {
            return Err(HealthError::Protocol);
        }
        let bytes = axum::body::to_bytes(Body::new(response.into_body()), MAX_HEALTH_BYTES)
            .await
            .map_err(|_| HealthError::Protocol)?;
        let health: Health = serde_json::from_slice(&bytes).map_err(|_| HealthError::Protocol)?;
        validate_build_version(&health.build_version).map_err(|_| HealthError::Protocol)?;
        if health.schema_version != SCHEMA_VERSION {
            return Err(HealthError::Protocol);
        }
        if expected_build.is_some_and(|expected| expected != health.build_version) {
            return Err(HealthError::BuildMismatch);
        }
        Ok(health)
    };
    let (result, transport) = tokio::join!(operation, connection);
    let health = result?;
    transport.map_err(|_| HealthError::Unavailable)?;
    Ok(health)
}
pub(super) async fn check(
    config: &CenterConfig,
    expected_build: Option<&str>,
) -> Result<Health, HealthError> {
    if let Some(value) = expected_build {
        validate_build_version(value).map_err(|_| HealthError::Configuration)?;
    }
    let tls = tls(config)?;
    tokio::time::timeout(HEALTH_DEADLINE, request(config, tls, expected_build))
        .await
        .map_err(|_| HealthError::Unavailable)?
}
