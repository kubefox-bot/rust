use super::{AccountError, AccountService};
use crate::{
    auth::SessionLease,
    vault::{self, AccountMaterial, RecoveryKey, SecretText, SystemEntropy},
};
use hmail_cleaner_protocol::browser::auth::RecoveryCandidate;
use std::{sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use tokio::time::Instant;
use uuid::Uuid;
const CANDIDATE_LIFETIME: Duration = Duration::from_secs(600);
pub(super) struct Pending {
    id: Uuid,
    generation: u64,
    revision: i64,
    deadline: Instant,
    fingerprint: ring::digest::Digest,
    material: AccountMaterial,
    cleanup: tokio::task::AbortHandle,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cleanup.abort();
    }
}
fn fingerprint(key: &RecoveryKey) -> ring::digest::Digest {
    ring::digest::digest(&ring::digest::SHA256, key.expose().as_bytes())
}
impl AccountService {
    pub async fn prepare_recovery(
        &self,
        lease: &SessionLease,
        password: SecretText,
    ) -> Result<RecoveryCandidate, AccountError> {
        let account = self.current(lease).await?;
        let (material, recovery) = self
            .store
            .kdf
            .try_run(move || {
                let dek = vault::open_password(&account.material, &password)?;
                let key = RecoveryKey::generate(&SystemEntropy)?;
                Ok((
                    vault::replace_recovery(&account.material, &dek, &key, &SystemEntropy)?,
                    key,
                ))
            })
            .await
            .map_err(AccountError::from_kdf)?;
        let _barrier = self.authority.credential_barrier.lock().await;
        self.current(lease).await?;
        let id = Uuid::new_v4();
        let deadline = Instant::now() + CANDIDATE_LIFETIME;
        let weak = Arc::downgrade(&self.candidate);
        let mut revoked = lease.revocation();
        let mut slot = self
            .candidate
            .lock()
            .map_err(|_| AccountError::Unavailable)?;
        let cleanup = tokio::spawn(async move {
            tokio::select! { _ = tokio::time::sleep_until(deadline) => {}, _ = revoked.wait_for(|value| *value) => {} }
            if let Some(owner) = weak.upgrade() {
                if let Ok(mut slot) = owner.lock() {
                    if slot.as_ref().is_some_and(|value| value.id == id) {
                        slot.take();
                    }
                }
            }
        });
        *slot = Some(Pending {
            id,
            generation: lease.generation(),
            revision: lease.account_revision(),
            deadline,
            fingerprint: fingerprint(&recovery),
            material,
            cleanup: cleanup.abort_handle(),
        });
        Ok(RecoveryCandidate {
            candidate_id: id,
            recovery_key: recovery.expose(),
            expires_in_seconds: CANDIDATE_LIFETIME.as_secs() as u16,
        })
    }
    pub async fn confirm_recovery(
        &self,
        lease: &SessionLease,
        id: Uuid,
        key: &RecoveryKey,
    ) -> Result<(), AccountError> {
        self.current(lease).await?;
        let material = {
            let mut slot = self
                .candidate
                .lock()
                .map_err(|_| AccountError::Unavailable)?;
            let pending = slot.as_ref().ok_or(AccountError::Invalid)?;
            if pending.deadline <= Instant::now()
                || pending.generation != lease.generation()
                || pending.revision != lease.account_revision()
            {
                slot.take();
                return Err(AccountError::Stale);
            }
            if pending.id != id
                || !bool::from(
                    pending
                        .fingerprint
                        .as_ref()
                        .ct_eq(fingerprint(key).as_ref()),
                )
            {
                return Err(AccountError::Invalid);
            }
            let pending = slot.take().ok_or(AccountError::Invalid)?;
            pending.material.clone()
        };
        self.commit(lease, material).await
    }
}
