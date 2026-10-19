use super::EnrollmentError;
use hmail_cleaner_protocol::{browser::enrollment::AgentPackageView, PROTOCOL_VERSION};
use serde::Deserialize;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
pub(crate) const EXECUTABLE: &str = "hmail-cleaner-agent.exe";
pub(super) const TRANSFER_BYTES: usize = 32 * 1024;
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 8192;
const MAX_PACKAGES: usize = 32;
#[derive(Clone)]
pub struct PackageCatalog {
    root: PathBuf,
}
pub struct AgentPackage {
    pub(super) file: tokio::fs::File,
    pub(super) manifest: AgentPackageView,
    pub(super) size: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    product: String,
    build: String,
    target: String,
    protocol_min: u16,
    protocol_max: u16,
    sha256: String,
}
pub(super) fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && !value.contains("..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'))
}
impl PackageCatalog {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub async fn list(&self) -> Result<Vec<AgentPackageView>, EnrollmentError> {
        let mut entries = match tokio::fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(EnrollmentError::Package),
        };
        let mut packages = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|_| EnrollmentError::Package)?
        {
            if packages.len() >= MAX_PACKAGES {
                return Err(EnrollmentError::Package);
            }
            let version = entry
                .file_name()
                .into_string()
                .map_err(|_| EnrollmentError::Package)?;
            packages.push(self.manifest(&version).await?);
        }
        packages.sort_by(|a, b| a.build.cmp(&b.build));
        Ok(packages)
    }
    async fn manifest(&self, version: &str) -> Result<AgentPackageView, EnrollmentError> {
        if !valid_version(version) {
            return Err(EnrollmentError::Package);
        }
        let directory = self.root.join(version);
        if !tokio::fs::symlink_metadata(&directory)
            .await
            .map_err(|_| EnrollmentError::Package)?
            .is_dir()
        {
            return Err(EnrollmentError::Package);
        }
        let path = directory.join("manifest.json");
        let metadata = tokio::fs::symlink_metadata(&path)
            .await
            .map_err(|_| EnrollmentError::Package)?;
        if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
            return Err(EnrollmentError::Package);
        }
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|_| EnrollmentError::Package)?;
        let mut bytes = Vec::new();
        file.take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| EnrollmentError::Package)?;
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(EnrollmentError::Package);
        }
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|_| EnrollmentError::Package)?;
        if manifest.product != "hmail-cleaner-agent"
            || manifest.target != "x86_64-pc-windows-msvc"
            || manifest.build != version
            || manifest.protocol_min > PROTOCOL_VERSION
            || manifest.protocol_max < PROTOCOL_VERSION
            || manifest.sha256.len() != 64
            || !manifest
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(EnrollmentError::Package);
        }
        Ok(AgentPackageView {
            build: manifest.build,
            sha256: manifest.sha256,
        })
    }
    pub async fn open(&self, version: &str) -> Result<AgentPackage, EnrollmentError> {
        let manifest = self.manifest(version).await?;
        let path = self.root.join(version).join(EXECUTABLE);
        let metadata = tokio::fs::symlink_metadata(&path)
            .await
            .map_err(|_| EnrollmentError::Package)?;
        if !metadata.is_file() || metadata.len() < 2 || metadata.len() > MAX_PACKAGE_BYTES {
            return Err(EnrollmentError::Package);
        }
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|_| EnrollmentError::Package)?;
        let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
        let mut buffer = [0u8; TRANSFER_BYTES];
        let mut size = 0;
        loop {
            let count = file
                .read(&mut buffer)
                .await
                .map_err(|_| EnrollmentError::Package)?;
            if count == 0 {
                break;
            }
            if size == 0 && !buffer[..count].starts_with(b"MZ") {
                return Err(EnrollmentError::Package);
            }
            size += count as u64;
            if size > MAX_PACKAGE_BYTES {
                return Err(EnrollmentError::Package);
            }
            hash.update(&buffer[..count]);
        }
        let actual = hash
            .finish()
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual != manifest.sha256 || size != metadata.len() {
            return Err(EnrollmentError::Package);
        }
        file.rewind().await.map_err(|_| EnrollmentError::Package)?;
        Ok(AgentPackage {
            file,
            manifest,
            size,
        })
    }
}
impl AgentPackage {
    pub fn metadata(&self) -> &AgentPackageView {
        &self.manifest
    }
}
