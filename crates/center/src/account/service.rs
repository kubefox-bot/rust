use super::Account;
use crate::{
    auth::{SessionAuthority, SessionLease},
    storage::CenterStore,
    vault::{AccountMaterial, KdfError},
};
use std::sync::{Arc, Mutex};
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("credentials or confirmation rejected")]
    Invalid,
    #[error("session or account revision changed")]
    Stale,
    #[error("another key derivation is running")]
    Busy,
    #[error("account operation failed")]
    Unavailable,
}
impl AccountError {
    pub(super) fn from_kdf(error: KdfError) -> Self {
        match error {
            KdfError::Busy => Self::Busy,
            KdfError::Vault(_) => Self::Invalid,
            KdfError::Worker => Self::Unavailable,
        }
    }
}
#[derive(Clone)]
pub struct AccountService {
    pub(super) store: CenterStore,
    pub(super) authority: Arc<SessionAuthority>,
    pub(super) candidate: Arc<Mutex<Option<super::recovery::Pending>>>,
}
impl AccountService {
    pub fn new(store: CenterStore, authority: Arc<SessionAuthority>) -> Self {
        Self {
            store,
            authority,
            candidate: Arc::new(Mutex::new(None)),
        }
    }
    pub(super) async fn current(&self, lease: &SessionLease) -> Result<Account, AccountError> {
        if !lease.is_live() {
            return Err(AccountError::Stale);
        }
        let account = self
            .store
            .account()
            .await
            .map_err(|_| AccountError::Unavailable)?
            .filter(|a| a.revision == lease.account_revision())
            .ok_or(AccountError::Stale)?;
        if !lease.is_live() {
            return Err(AccountError::Stale);
        }
        Ok(account)
    }
    pub(super) async fn commit(
        &self,
        lease: &SessionLease,
        material: AccountMaterial,
    ) -> Result<(), AccountError> {
        let owner = self.clone();
        let lease = lease.clone();
        // Own the short commit and RAM revision update even when the HTTP waiter disappears.
        tokio::spawn(async move {
            let _barrier = owner.authority.credential_barrier.lock().await;
            owner.current(&lease).await?;
            let revision = lease.account_revision();
            let next = revision.checked_add(1).ok_or(AccountError::Unavailable)?;
            let encoded = material.encode().map_err(|_| AccountError::Unavailable)?;
            if !owner.authority.revoke(lease.generation()) {
                return Err(AccountError::Stale);
            }
            let mut transaction = owner
                .store
                .pool
                .begin()
                .await
                .map_err(|_| AccountError::Unavailable)?;
            let result = sqlx::query(
                "UPDATE account SET material=?,revision=? WHERE singleton=1 AND revision=?",
            )
            .bind(encoded)
            .bind(next)
            .bind(revision)
            .execute(&mut *transaction)
            .await
            .map_err(|_| AccountError::Unavailable)?;
            if result.rows_affected() != 1 {
                return Err(AccountError::Stale);
            }
            transaction
                .commit()
                .await
                .map_err(|_| AccountError::Unavailable)?;
            owner
                .authority
                .advance_revision(revision, next)
                .map_err(|_| AccountError::Unavailable)
        })
        .await
        .map_err(|_| AccountError::Unavailable)?
    }
}
