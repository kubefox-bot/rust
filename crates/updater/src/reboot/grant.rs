use crate::{journal, Error};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Pending,
    Granted,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Grant {
    pub version: u8,
    pub operation: Uuid,
    pub boot_id: Uuid,
    pub phase: Phase,
}
pub(super) fn load(path: &Path) -> Result<Option<Grant>, Error> {
    let grant = journal::load::<Grant>(path)?;
    if grant.as_ref().is_some_and(|grant| {
        grant.version != 1 || grant.operation.is_nil() || grant.boot_id.is_nil()
    }) {
        return Err(Error::Invalid);
    }
    Ok(grant)
}
pub(super) fn remove(path: &Path) -> Result<(), Error> {
    std::fs::remove_file(path).map_err(|_| Error::Storage)?;
    std::fs::File::open(path.parent().ok_or(Error::Invalid)?)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| Error::Storage)
}
