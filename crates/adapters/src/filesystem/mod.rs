mod delete;
mod orphans;
mod prune;
mod removal;
mod retry;
mod scan;
mod validation;
mod workers;

use hmail_cleaner_core::cleanup::{FileOutcome, FileStore, FileTarget, PortResult, PruneOutcome};
use hmail_cleaner_core::maintenance::{
    FileObservation, FullFileStore, OrphanFileOutcome, ScanStats,
};
use std::path::PathBuf;

pub struct LocalFiles {
    root: PathBuf,
}
impl FullFileStore for LocalFiles {
    type Scan = scan::Session;
    async fn validate_tree(&self) -> PortResult<ScanStats> {
        scan::check(self.root.clone()).await
    }
    async fn open_scan(&self) -> PortResult<Self::Scan> {
        scan::open(self.root.clone()).await
    }
    async fn delete_orphans(
        &self,
        files: Vec<FileObservation>,
        workers: u8,
    ) -> PortResult<Vec<OrphanFileOutcome>> {
        if !(1..=hmail_cleaner_core::cleanup::MAX_WORKERS).contains(&workers) {
            return Err("workers must be between 1 and 8".into());
        }
        orphans::run(self.root.clone(), files, workers).await
    }
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
