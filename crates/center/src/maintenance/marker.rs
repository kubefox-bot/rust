use super::MaintenanceError;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;
const FILE: &str = "maintenance.json";
const MAX_BYTES: u64 = 256;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    version: u8,
    id: Uuid,
}
pub(super) fn load(directory: &Path) -> Result<Option<Uuid>, MaintenanceError> {
    let path = directory.join(FILE);
    let meta = match path.symlink_metadata() {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(MaintenanceError::Unavailable),
    };
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err(MaintenanceError::Unavailable);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|f| f.take(MAX_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|_| MaintenanceError::Unavailable)?;
    let marker: Marker =
        serde_json::from_slice(&bytes).map_err(|_| MaintenanceError::Unavailable)?;
    if marker.version != 1 || marker.id.is_nil() {
        return Err(MaintenanceError::Unavailable);
    }
    Ok(Some(marker.id))
}
pub(super) fn create(directory: &Path, id: Uuid) -> Result<(), MaintenanceError> {
    let temporary: PathBuf = directory.join(format!(".maintenance-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| MaintenanceError::Unavailable)?;
        let bytes = serde_json::to_vec(&Marker { version: 1, id })
            .map_err(|_| MaintenanceError::Unavailable)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| MaintenanceError::Unavailable)?;
        fs::rename(&temporary, directory.join(FILE)).map_err(|_| MaintenanceError::Unavailable)?;
        sync(directory)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
pub(super) fn remove(directory: &Path) -> Result<(), MaintenanceError> {
    fs::remove_file(directory.join(FILE)).map_err(|_| MaintenanceError::Unavailable)?;
    sync(directory)
}
fn sync(directory: &Path) -> Result<(), MaintenanceError> {
    #[cfg(unix)]
    fs::File::open(directory)
        .and_then(|f| f.sync_all())
        .map_err(|_| MaintenanceError::Unavailable)?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}
