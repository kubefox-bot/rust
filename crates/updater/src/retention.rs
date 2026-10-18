use crate::{
    config::{private_directory, Config},
    journal::{self, Phase, Transaction},
    podman, Error,
};
/// Only transaction-owned successful backups are eligible; unresolved incidents are retained.
pub fn expired(mut history: Vec<Transaction>) -> Vec<Transaction> {
    history.retain(|tx| tx.phase == Phase::Committed);
    history.sort_by_key(|tx| std::cmp::Reverse(tx.sequence));
    history.into_iter().skip(2).collect()
}
pub fn record(config: &Config, tx: &Transaction) -> Result<(), Error> {
    let directory = config.state_dir.join("history");
    private_directory(&directory)?;
    journal::save(&directory.join(format!("{}.json", tx.id)), tx)
}
pub async fn cleanup(config: &Config, current: &Transaction) -> Result<(), Error> {
    let directory = config.state_dir.join("history");
    let mut history = Vec::new();
    for item in std::fs::read_dir(&directory).map_err(|_| Error::Storage)? {
        let item = item.map_err(|_| Error::Storage)?;
        if item.file_type().map_err(|_| Error::Storage)?.is_file() {
            let tx = journal::load::<Transaction>(&item.path())?.ok_or(Error::Invalid)?;
            history.push(tx);
        }
    }
    let protected: Vec<_> = history
        .iter()
        .filter(|tx| {
            matches!(
                tx.phase,
                Phase::Quarantined
                    | Phase::Restoring
                    | Phase::RollbackExposing
                    | Phase::PreviousExposing
            )
        })
        .flat_map(|tx| [tx.previous_image.clone(), tx.candidate_image.clone()])
        .chain([
            current.previous_image.clone(),
            current.candidate_image.clone(),
        ])
        .collect();
    for old in expired(history) {
        if old.id == current.id {
            continue;
        }
        let parent = config.backup_dir.join(old.id.to_string());
        if old.backup != parent.join("snapshot")
            || old.candidate_export != parent.join("candidate.tar")
            || old.previous_unit
                != config
                    .state_dir
                    .join(format!("previous-{}.container", old.id))
        {
            return Err(Error::Invalid);
        }
        if parent.try_exists().map_err(|_| Error::Storage)? {
            let metadata = std::fs::symlink_metadata(&parent).map_err(|_| Error::Storage)?;
            if !metadata.is_dir() {
                return Err(Error::Invalid);
            }
            std::fs::remove_dir_all(&parent).map_err(|_| Error::Storage)?;
        }
        if old.previous_unit.try_exists().map_err(|_| Error::Storage)? {
            std::fs::remove_file(&old.previous_unit).map_err(|_| Error::Storage)?;
        }
        for image in [&old.previous_image, &old.candidate_image] {
            if protected.contains(image)
                || !image.starts_with("ghcr.io/kubefox-bot/astra-center@sha256:")
            {
                continue;
            }
            // Never force removal: other containers can legitimately reference a retained image.
            let _ = podman::podman(vec!["image".into(), "rm".into(), image.into()]).await;
        }
        std::fs::remove_file(directory.join(format!("{}.json", old.id)))
            .map_err(|_| Error::Storage)?;
    }
    Ok(())
}
