use super::*;
use std::{
    collections::HashSet,
    path::{Component, Path},
};
fn contained(root: &Path, path: &Path) -> bool {
    path.to_str().is_some()
        && path.strip_prefix(root).is_ok_and(|relative| {
            relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        })
}
pub(super) fn inventory(rows: &[StoredMessage], root: &Path) -> PortResult<()> {
    let unique = rows
        .iter()
        .map(|row| row.candidate.id)
        .collect::<HashSet<_>>();
    if rows.len() > MAX_REFERENCES
        || unique.len() != rows.len()
        || rows
            .iter()
            .any(|row| row.candidate.id <= 0 || row.path == root || !contained(root, &row.path))
    {
        return Err("Некорректная карта путей БД; очистка не начата".into());
    }
    Ok(())
}
pub(super) fn scan(batch: &ScanBatch, options: &RunOptions) -> PortResult<()> {
    let valid = batch.visited_entries <= u64::from(options.batch_size.get())
        && batch.examined_files <= batch.visited_entries
        && batch.files.len() as u64 <= batch.examined_files
        && (batch.finished || batch.visited_entries > 0)
        && batch.files.iter().all(|file| {
            file.path != options.data_directory
                && contained(&options.data_directory, &file.path)
                && file
                    .path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("eml"))
        })
        && batch
            .completed_directories
            .iter()
            .all(|path| contained(&options.data_directory, path));
    let unique = batch
        .files
        .iter()
        .map(|file| path_key(&file.path))
        .collect::<HashSet<_>>();
    if !valid || unique.len() != batch.files.len() {
        return Err("Некорректный пакет рекурсивного обхода".into());
    }
    Ok(())
}
