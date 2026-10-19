use hmail_cleaner_protocol::browser::updates::UpdatePolicy;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

const POLICY_FILENAME: &str = "update-policy.json";
const POLICY_FORMAT: u16 = 1;
const MAX_POLICY_BYTES: u64 = 4096;

#[derive(Debug, thiserror::Error)]
#[error("update policy is invalid or unavailable")]
pub struct PolicyError;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyFile {
    format_version: u16,
    policy: UpdatePolicy,
}

/// Absence means an older installation uses defaults. Corruption never does.
pub fn read_policy(data_dir: &Path) -> Result<UpdatePolicy, PolicyError> {
    let path = data_dir.join(POLICY_FILENAME);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(UpdatePolicy::default());
        }
        Err(_) => return Err(PolicyError),
    };
    if !metadata.is_file() || metadata.len() > MAX_POLICY_BYTES {
        return Err(PolicyError);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| PolicyError)?
        .take(MAX_POLICY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PolicyError)?;
    if bytes.len() as u64 > MAX_POLICY_BYTES {
        return Err(PolicyError);
    }
    let stored: PolicyFile = serde_json::from_slice(&bytes).map_err(|_| PolicyError)?;
    if stored.format_version != POLICY_FORMAT {
        return Err(PolicyError);
    }
    stored.policy.validate().map_err(|_| PolicyError)?;
    Ok(stored.policy)
}

/// The caller holds the center data lease, mutation admission, and credential barrier.
pub(super) fn write_policy(data_dir: &Path, policy: UpdatePolicy) -> Result<(), PolicyError> {
    policy.validate().map_err(|_| PolicyError)?;
    let bytes = serde_json::to_vec(&PolicyFile {
        format_version: POLICY_FORMAT,
        policy,
    })
    .map_err(|_| PolicyError)?;
    let temporary = data_dir.join(format!(".update-policy-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|_| PolicyError)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| PolicyError)?;
        drop(file);
        std::fs::rename(&temporary, data_dir.join(POLICY_FILENAME)).map_err(|_| PolicyError)?;
        #[cfg(unix)]
        File::open(data_dir)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| PolicyError)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
