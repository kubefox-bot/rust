use super::{AgentBootstrap, EnrollmentError};
use hmail_cleaner_protocol::AgentId;
use rcgen::KeyPair;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use zeroize::Zeroizing;

const PRIVATE_KEY_NAME: &str = "identity-key.pem";
const METADATA_NAME: &str = "agent-id.json";
const PRIVATE_KEY_LIMIT: u64 = 16 * 1024;
const METADATA_LIMIT: u64 = 1024;

pub(super) fn load_or_create(bootstrap: &AgentBootstrap) -> Result<KeyPair, EnrollmentError> {
    directory(&bootstrap.data_directory)?;
    let key_path = bootstrap.data_directory.join(PRIVATE_KEY_NAME);
    let metadata = bootstrap.data_directory.join(METADATA_NAME);
    match (key_path.exists(), metadata.exists()) {
        (true, true) => {
            let stored: AgentId = serde_json::from_slice(&read(&metadata, METADATA_LIMIT)?)
                .map_err(|_| EnrollmentError::Metadata)?;
            if stored != bootstrap.agent_id {
                return Err(EnrollmentError::IdentityConflict);
            }
            let info = std::fs::symlink_metadata(&key_path)?;
            if !info.is_file() || info.file_type().is_symlink() {
                return Err(EnrollmentError::Permissions);
            }
            private_permissions(&info)?;
            let bytes = Zeroizing::new(read(&key_path, PRIVATE_KEY_LIMIT)?);
            let pem = std::str::from_utf8(&bytes).map_err(|_| EnrollmentError::Metadata)?;
            Ok(KeyPair::from_pem(pem)?)
        }
        (false, false) => {
            let key = KeyPair::generate()?;
            let pem = Zeroizing::new(key.serialize_pem());
            create(&key_path, pem.as_bytes())?;
            create(
                &metadata,
                &serde_json::to_vec(&bootstrap.agent_id).map_err(|_| EnrollmentError::Metadata)?,
            )?;
            Ok(key)
        }
        (true, false) | (false, true) => Err(EnrollmentError::Partial),
    }
}

fn directory(path: &Path) -> Result<(), EnrollmentError> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    let info = std::fs::symlink_metadata(path)?;
    if !info.is_dir() || info.file_type().is_symlink() {
        return Err(EnrollmentError::Permissions);
    }
    private_permissions(&info)
}
fn private_permissions(metadata: &std::fs::Metadata) -> Result<(), EnrollmentError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(EnrollmentError::Permissions);
        }
    }
    #[cfg(not(unix))]
    let _ = metadata;
    Ok(())
}
fn create(path: &Path, bytes: &[u8]) -> Result<(), EnrollmentError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, EnrollmentError> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(EnrollmentError::Metadata);
    }
    Ok(bytes)
}
