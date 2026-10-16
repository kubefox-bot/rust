use super::super::validation;
use hmail_cleaner_core::{maintenance::FileObservation, paths::message_guid};
use std::{
    fs, io,
    path::{Component, Path},
};

pub(in crate::filesystem) fn observation(
    path: &Path,
    metadata: &fs::Metadata,
) -> io::Result<Option<FileObservation>> {
    if validation::linked(metadata)
        || !metadata.is_file()
        || !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("eml"))
    {
        return Ok(None);
    }
    if path.to_str().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Путь EML содержит неподдерживаемый Unicode",
        ));
    }
    Ok(Some(FileObservation {
        path: path.to_path_buf(),
        guid: path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| message_guid(n).ok())
            .unwrap_or_default(),
        size_bytes: metadata.len(),
        modified_at: metadata.modified()?,
        created_at: metadata.created()?,
    }))
}
pub(in crate::filesystem) fn allowed_path(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
            && relative.file_name().is_some()
            && relative
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("eml"))
    })
}
