use crate::{
    config::{Config, Credentials},
    Error,
};
use reqwest::{header, Client};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::{path::Path, time::Duration};
pub struct Network {
    client: Client,
    credentials: Credentials,
}
fn allowed(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.port().is_none_or(|port| port == 443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str().is_some_and(|host| {
            matches!(
                host,
                "api.github.com"
                    | "github.com"
                    | "objects.githubusercontent.com"
                    | "release-assets.githubusercontent.com"
            )
        })
}
impl Network {
    pub fn new(config: &Config) -> Result<Self, Error> {
        let credentials = Credentials::load(&config.credentials_file)?;
        let native = rustls_native_certs::load_native_certs();
        let mut roots = rustls::RootCertStore::empty();
        for certificate in native.certs {
            roots.add(certificate).map_err(|_| Error::Transport)?;
        }
        if roots.is_empty() {
            return Err(Error::Transport);
        }
        let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| Error::Transport)?
        .with_root_certificates(roots)
        .with_no_client_auth();
        let client = Client::builder()
            .tls_backend_preconfigured(tls)
            .https_only(true)
            .user_agent("astra-updater/1")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 || !allowed(attempt.url()) {
                    attempt.error("redirect outside release hosts")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(|_| Error::Transport)?;
        Ok(Self {
            client,
            credentials,
        })
    }
    async fn response(&self, url: &str, asset: bool) -> Result<reqwest::Response, Error> {
        let parsed = url::Url::parse(url).map_err(|_| Error::Invalid)?;
        if !allowed(&parsed) {
            return Err(Error::Invalid);
        }
        let request = self.client.get(parsed.clone()).header(
            header::ACCEPT,
            if asset {
                "application/octet-stream"
            } else {
                "application/vnd.github+json"
            },
        );
        let request = if parsed.host_str() == Some("api.github.com") {
            if let Some(token) = &self.credentials.github_token {
                request.bearer_auth(token)
            } else {
                request
            }
        } else {
            request
        };
        request
            .send()
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|_| Error::Transport)
    }
    pub async fn bytes(&self, url: &str, limit: usize, asset: bool) -> Result<Vec<u8>, Error> {
        let mut response = self.response(url, asset).await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Transport)? {
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err(Error::Invalid);
            }
            bytes.extend_from_slice(&chunk)
        }
        Ok(bytes)
    }
    pub async fn file(&self, url: &str, path: &Path, limit: u64) -> Result<(), Error> {
        let mut response = self.response(url, true).await?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|_| Error::Storage)?;
        let mut length = 0u64;
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Transport)? {
            length = length
                .checked_add(chunk.len() as u64)
                .ok_or(Error::Invalid)?;
            if length > limit {
                return Err(Error::Invalid);
            }
            file.write_all(&chunk).map_err(|_| Error::Storage)?;
        }
        file.sync_all().map_err(|_| Error::Storage)
    }
}
