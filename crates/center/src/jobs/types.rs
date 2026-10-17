use crate::storage::CenterStore;
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub struct JobService {
    pub(crate) store: CenterStore,
    pub(crate) events: super::EventHub,
    pub(crate) epoch: Uuid,
    pub(super) operation: Arc<tokio::sync::Mutex<()>>,
    pub(super) follower: Arc<tokio::sync::Mutex<()>>,
    pub(super) follower_generation: Arc<std::sync::Mutex<Option<u64>>>,
}
impl JobService {
    pub fn new(store: CenterStore) -> Self {
        let events = super::EventHub::default();
        let epoch = events.epoch();
        Self {
            store,
            events,
            epoch,
            operation: Arc::new(tokio::sync::Mutex::new(())),
            follower: Arc::new(tokio::sync::Mutex::new(())),
            follower_generation: Arc::new(std::sync::Mutex::new(None)),
        }
    }
    pub fn events(&self) -> &super::EventHub {
        &self.events
    }
}
#[derive(Debug, thiserror::Error)]
pub enum SubmitError {
    #[error("another operation is reserved")]
    Busy,
    #[error("job parameters conflict")]
    Conflict,
    #[error("invalid or unconfirmed job")]
    Invalid,
    #[error("job not found")]
    NotFound,
    #[error("job storage unavailable")]
    Storage,
    #[error("session revoked")]
    Unauthorized,
    #[error("invalid agent response")]
    Protocol,
}
impl From<sqlx::Error> for SubmitError {
    fn from(_: sqlx::Error) -> Self {
        Self::Storage
    }
}
impl From<serde_json::Error> for SubmitError {
    fn from(_: serde_json::Error) -> Self {
        Self::Protocol
    }
}
impl From<crate::profiles::ProfileError> for SubmitError {
    fn from(error: crate::profiles::ProfileError) -> Self {
        use crate::profiles::ProfileError;
        match error {
            ProfileError::Unauthorized => Self::Unauthorized,
            ProfileError::NotFound => Self::NotFound,
            ProfileError::Conflict => Self::Conflict,
            ProfileError::Invalid => Self::Invalid,
            ProfileError::Storage => Self::Storage,
        }
    }
}
