use super::marker;
use crate::storage::CenterStore;
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;
#[derive(Debug, thiserror::Error)]
pub enum MaintenanceError {
    #[error("maintenance operation conflicts")]
    Conflict,
    #[error("maintenance state unavailable")]
    Unavailable,
    #[error("maintenance request invalid")]
    Invalid,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceStatus {
    Idle,
    Draining,
    Ready,
}
#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub version: u8,
    pub status: MaintenanceStatus,
    pub active_jobs: u64,
    pub active_enrollments: u64,
    pub active_writers: u64,
    pub vault_id: Option<Uuid>,
    pub policy: hmail_cleaner_protocol::browser::updates::UpdatePolicy,
}
enum Admission {
    New,
    Continuation,
}
struct State {
    operation: Option<Uuid>,
    ready: bool,
    writers: u64,
    revision: u64,
    failed: bool,
}
struct Owner {
    directory: PathBuf,
    state: Mutex<State>,
}
#[derive(Clone)]
pub struct MaintenanceGate(Arc<Owner>);
/// Owned permits must travel into detached work, rather than stay with its HTTP waiter.
pub struct WritePermit(Arc<Owner>);
impl Drop for WritePermit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.writers = state.writers.saturating_sub(1);
            state.revision = state.revision.wrapping_add(1);
        }
    }
}
impl MaintenanceGate {
    pub(crate) fn open(directory: &Path) -> Result<Self, MaintenanceError> {
        Ok(Self(Arc::new(Owner {
            directory: directory.into(),
            state: Mutex::new(State {
                operation: marker::load(directory)?,
                ready: false,
                writers: 0,
                revision: 0,
                failed: false,
            }),
        })))
    }
    pub fn admit(&self) -> Result<WritePermit, MaintenanceError> {
        self.permit(Admission::New)
    }
    /// Existing remote operations can complete during draining; sealed readiness blocks all writes.
    pub(crate) fn continuation(&self) -> Result<WritePermit, MaintenanceError> {
        self.permit(Admission::Continuation)
    }
    fn permit(&self, admission: Admission) -> Result<WritePermit, MaintenanceError> {
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| MaintenanceError::Unavailable)?;
        if state.failed {
            return Err(MaintenanceError::Unavailable);
        }
        if state.ready || (state.operation.is_some() && matches!(admission, Admission::New)) {
            return Err(MaintenanceError::Conflict);
        }
        state.writers = state
            .writers
            .checked_add(1)
            .ok_or(MaintenanceError::Unavailable)?;
        state.revision = state.revision.wrapping_add(1);
        Ok(WritePermit(self.0.clone()))
    }
    pub fn begin(&self, id: Uuid) -> Result<(), MaintenanceError> {
        if id.is_nil() {
            return Err(MaintenanceError::Invalid);
        }
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| MaintenanceError::Unavailable)?;
        if state.failed {
            return Err(MaintenanceError::Unavailable);
        }
        if let Some(previous) = state.operation {
            return if previous == id {
                Ok(())
            } else {
                Err(MaintenanceError::Conflict)
            };
        }
        // Fail closed on persistence failure: an operator must resolve the marker before reopening.
        state.operation = Some(id);
        if marker::create(&self.0.directory, id).is_err() {
            state.failed = true;
            return Err(MaintenanceError::Unavailable);
        }
        Ok(())
    }
    pub fn end(&self, id: Uuid) -> Result<(), MaintenanceError> {
        if id.is_nil() {
            return Err(MaintenanceError::Invalid);
        }
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| MaintenanceError::Unavailable)?;
        if state.failed {
            return Err(MaintenanceError::Unavailable);
        }
        match state.operation {
            Some(current) if current != id => return Err(MaintenanceError::Conflict),
            None => return Ok(()),
            Some(_) => (),
        }
        if marker::remove(&self.0.directory).is_err() {
            state.failed = true;
            return Err(MaintenanceError::Unavailable);
        }
        state.operation = None;
        state.ready = false;
        state.revision = state.revision.wrapping_add(1);
        Ok(())
    }
    pub(super) fn verify(&self, id: Uuid) -> Result<(), MaintenanceError> {
        if id.is_nil() {
            return Err(MaintenanceError::Invalid);
        }
        let state = self
            .0
            .state
            .lock()
            .map_err(|_| MaintenanceError::Unavailable)?;
        if state.operation.is_some_and(|current| current != id) {
            return Err(MaintenanceError::Conflict);
        }
        Ok(())
    }
    pub async fn status(&self, store: &CenterStore) -> Result<Snapshot, MaintenanceError> {
        let revision = self
            .0
            .state
            .lock()
            .map_err(|_| MaintenanceError::Unavailable)?
            .revision;
        // One snapshot query sees both durable slots. Unknown/lost receipts keep their slot busy.
        let (jobs,enrollments,vault): (bool,bool,Option<String>) = sqlx::query_as("SELECT (SELECT job_id IS NOT NULL FROM job_slot WHERE singleton=1), (SELECT enrollment_id IS NOT NULL FROM provisioning_slot WHERE singleton=1), (SELECT vault_id FROM identity WHERE singleton=1)")
            .fetch_one(&store.pool).await.map_err(|_| MaintenanceError::Unavailable)?;
        let vault_id = vault
            .map(|value| Uuid::parse_str(&value).map_err(|_| MaintenanceError::Unavailable))
            .transpose()?;
        let policy = crate::update_settings::read_policy(&self.0.directory)
            .map_err(|_| MaintenanceError::Unavailable)?;
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| MaintenanceError::Unavailable)?;
        if state.failed {
            return Err(MaintenanceError::Unavailable);
        }
        if state.operation.is_some()
            && state.writers == 0
            && !jobs
            && !enrollments
            && state.revision == revision
        {
            state.ready = true;
        }
        Ok(Snapshot {
            version: 1,
            status: if state.operation.is_none() {
                MaintenanceStatus::Idle
            } else if state.ready {
                MaintenanceStatus::Ready
            } else {
                MaintenanceStatus::Draining
            },
            active_jobs: u64::from(jobs),
            active_enrollments: u64::from(enrollments),
            active_writers: state.writers,
            vault_id,
            policy,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn admission_revision_closes_slot_observation_gap() {
        let root = tempfile::tempdir().unwrap();
        let lease = crate::storage::DataDirectory::acquire(root.path()).unwrap();
        let store = CenterStore::open(&lease).await.unwrap();
        store.maintenance().begin(Uuid::new_v4()).unwrap();
        let first = store.pool.acquire().await.unwrap();
        let second = store.pool.acquire().await.unwrap();
        let status = store.maintenance().status(&store);
        tokio::pin!(status);
        assert!(futures_util::poll!(&mut status).is_pending());
        // This writer starts and finishes after the query's revision was captured.
        // A stale empty-slot observation must not seal readiness.
        let admitted = store.maintenance().continuation().unwrap();
        drop(admitted);
        drop(first);
        drop(second);
        assert_eq!(status.await.unwrap().status, MaintenanceStatus::Draining);
        assert_eq!(
            store.maintenance().status(&store).await.unwrap().status,
            MaintenanceStatus::Ready
        );
    }
    #[tokio::test]
    async fn local_snapshot_reads_vault_identity_from_owned_pool_and_rejects_corruption() {
        let root = tempfile::tempdir().unwrap();
        let lease = crate::storage::DataDirectory::acquire(root.path()).unwrap();
        let store = CenterStore::open(&lease).await.unwrap();
        assert_eq!(
            store.maintenance().status(&store).await.unwrap().vault_id,
            None
        );
        let vault = Uuid::new_v4();
        sqlx::query("INSERT INTO identity(singleton,vault_id) VALUES(1,?)")
            .bind(vault.to_string())
            .execute(&store.pool)
            .await
            .unwrap();
        assert_eq!(
            store.maintenance().status(&store).await.unwrap().vault_id,
            Some(vault)
        );
        sqlx::query("UPDATE identity SET vault_id='corrupt' WHERE singleton=1")
            .execute(&store.pool)
            .await
            .unwrap();
        assert!(matches!(
            store.maintenance().status(&store).await,
            Err(MaintenanceError::Unavailable)
        ));
    }
}
