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
pub(super) fn groups(groups: &[StoredGroup], root: &Path, limit: usize) -> PortResult<()> {
    let unique = groups
        .iter()
        .map(|group| group.row.candidate.id)
        .collect::<HashSet<_>>();
    let paths = groups
        .iter()
        .map(|group| path_key(&group.row.path))
        .collect::<HashSet<_>>();
    if groups.len() > limit
        || unique.len() != groups.len()
        || paths.len() != groups.len()
        || groups.iter().any(|group| {
            group.references == 0
                || group.row.candidate.id <= 0
                || group.row.path == root
                || !contained(root, &group.row.path)
        })
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
