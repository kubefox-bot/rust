use crate::Error;
use serde::Deserialize;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    io::Read,
    path::{Component, Path, PathBuf},
};
pub const MAX_CONFIG_BYTES: u64 = 16384;
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub repository: String,
    pub image_repository: String,
    pub state_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub backup_dir: PathBuf,
    pub unit_path: PathBuf,
    pub socket_path: PathBuf,
    pub cosign: PathBuf,
    pub trust_root: PathBuf,
    pub credentials_file: PathBuf,
    pub registry_authfile: PathBuf,
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let config: Self = serde_json::from_slice(&private_read(path, MAX_CONFIG_BYTES)?)
            .map_err(|_| Error::Invalid)?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.repository != "kubefox-bot/rust"
            || self.image_repository != "ghcr.io/kubefox-bot/astra-center"
        {
            return Err(Error::Invalid);
        }
        for path in [
            &self.state_dir,
            &self.staging_dir,
            &self.backup_dir,
            &self.unit_path,
            &self.socket_path,
            &self.cosign,
            &self.trust_root,
            &self.credentials_file,
            &self.registry_authfile,
        ] {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
            {
                return Err(Error::Invalid);
            }
        }
        if self
            .unit_path
            .file_name()
            .is_none_or(|name| name != "hmail-center.container")
        {
            return Err(Error::Invalid);
        }
        if self.state_dir == self.staging_dir
            || self.state_dir == self.backup_dir
            || self.staging_dir == self.backup_dir
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn prepare(&self) -> Result<(), Error> {
        for path in [&self.state_dir, &self.staging_dir, &self.backup_dir] {
            private_directory(path)?;
        }
        Ok(())
    }
}
/// Read only bounded regular files owned by this account; do not follow a final symlink.
pub fn private_read(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| Error::Storage)?;
    let metadata = file.metadata().map_err(|_| Error::Storage)?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > limit
    {
        return Err(Error::Invalid);
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Storage)?;
    if bytes.len() as u64 > limit {
        return Err(Error::Invalid);
    }
    Ok(bytes)
}
pub fn private_directory(path: &Path) -> Result<(), Error> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if !meta.is_dir()
                || meta.uid() != unsafe { libc::geteuid() }
                || meta.permissions().mode() & 0o077 != 0
            {
                return Err(Error::Invalid);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(path).map_err(|_| Error::Storage)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| Error::Storage)?;
        }
        Err(_) => return Err(Error::Storage),
    }
    Ok(())
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    pub github_token: Option<String>,
}
impl Credentials {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let credentials: Self = serde_json::from_slice(&private_read(path, MAX_CONFIG_BYTES)?)
            .map_err(|_| Error::Invalid)?;
        if credentials.github_token.as_ref().is_some_and(|token| {
            token.is_empty() || token.len() > 4096 || !token.bytes().all(|b| b.is_ascii_graphic())
        }) {
            return Err(Error::Invalid);
        }
        Ok(credentials)
    }
}
