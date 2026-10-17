use hmail_cleaner_adapters::mtls::{
    self, ClientIdentity, PeerIdentity, RevokedSerials, ServerTls, TrustPolicy,
};
use hmail_cleaner_protocol::AgentId;
use serde::Deserialize;
use std::{
    fs::File,
    io::Read,
    net::SocketAddr,
    path::{Path, PathBuf},
};
use tokio_rustls::rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use uuid::Uuid;
use zeroize::Zeroizing;
const BOOTSTRAP_LIMIT: u64 = 16 * 1024;
const CERTIFICATE_LIMIT: u64 = 64 * 1024;
const PRIVATE_KEY_LIMIT: u64 = 16 * 1024;

pub struct AgentConfig {
    pub listen: SocketAddr,
    pub data_directory: PathBuf,
    pub agent_id: AgentId,
    pub build_version: String,
    pub tls: ServerTls,
}
#[derive(Debug, thiserror::Error)]
#[error("invalid or unavailable agent bootstrap configuration")]
pub struct ConfigError;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bootstrap {
    listen: SocketAddr,
    agent_id: AgentId,
    center_id: Uuid,
}
impl AgentConfig {
    pub fn load(path: &Path, build_version: &str) -> Result<Self, ConfigError> {
        if !path.is_absolute() {
            return Err(ConfigError);
        }
        let data_directory = path.parent().ok_or(ConfigError)?.to_path_buf();
        let bootstrap: Bootstrap =
            serde_json::from_slice(&read(path, BOOTSTRAP_LIMIT)?).map_err(|_| ConfigError)?;
        if bootstrap.listen.port() == 0 || bootstrap.center_id.is_nil() {
            return Err(ConfigError);
        }
        let stored_id: AgentId = serde_json::from_slice(&read(
            &data_directory.join("agent-id.json"),
            BOOTSTRAP_LIMIT,
        )?)
        .map_err(|_| ConfigError)?;
        if stored_id != bootstrap.agent_id {
            return Err(ConfigError);
        }
        let roots = certificates(&data_directory.join("trust.pem"))?;
        let chain = certificates(&data_directory.join("identity.pem"))?;
        let private = Zeroizing::new(read(
            &data_directory.join("identity-key.pem"),
            PRIVATE_KEY_LIMIT,
        )?);
        let key = PrivateKeyDer::from_pem_slice(&private).map_err(|_| ConfigError)?;
        let identity = ClientIdentity {
            certificates: chain,
            private_key: key,
        };
        let revoked = RevokedSerials::default();
        let config = mtls::server_config(
            &identity,
            &TrustPolicy { roots },
            PeerIdentity::Center(bootstrap.center_id),
            &revoked,
        )
        .map_err(|_| ConfigError)?;
        Ok(Self {
            listen: bootstrap.listen,
            data_directory,
            agent_id: bootstrap.agent_id,
            build_version: build_version.into(),
            tls: ServerTls::new(config, revoked),
        })
    }
}
fn certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>, ConfigError> {
    let bytes = read(path, CERTIFICATE_LIMIT)?;
    let certificates = CertificateDer::pem_slice_iter(&bytes)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ConfigError)?;
    if certificates.is_empty() {
        return Err(ConfigError);
    }
    Ok(certificates)
}
fn read(path: &Path, maximum: u64) -> Result<Vec<u8>, ConfigError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ConfigError)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ConfigError);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| ConfigError)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ConfigError)?;
    if bytes.len() as u64 > maximum {
        return Err(ConfigError);
    }
    Ok(bytes)
}
