mod delete;
mod prune;
mod validation;
mod workers;

use hmail_cleaner_core::cleanup::{FileOutcome, FileStore, FileTarget, PortResult, PruneOutcome};
use std::path::PathBuf;

pub struct LocalFiles {
    root: PathBuf,
}
impl LocalFiles {
    pub fn new(root: PathBuf) -> PortResult<Self> {
        if !root.is_absolute() {
            return Err("DataFolder must be absolute".into());
        }
        validation::directory(&root).map_err(|error| error.message)?;
        Ok(Self { root })
    }
}
impl FileStore for LocalFiles {
    async fn delete(&self, files: Vec<FileTarget>, workers: u8) -> PortResult<Vec<FileOutcome>> {
        if !(1..=hmail_cleaner_core::cleanup::MAX_WORKERS).contains(&workers) {
            return Err("workers must be between 1 and 8".into());
        }
        delete::run(self.root.clone(), files, workers).await
    }
    async fn prune_empty(&self, parents: Vec<PathBuf>) -> PortResult<PruneOutcome> {
        let root = self.root.clone();
        Ok(tokio::task::spawn_blocking(move || prune::run(&root, parents)).await?)
    }
}
