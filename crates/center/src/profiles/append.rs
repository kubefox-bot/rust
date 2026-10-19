use super::{ProfileError, ProfileService};
use crate::auth::SessionLease;
use hmail_cleaner_protocol::{ProfileId, ServerId};
pub(super) struct NewVersion {
    pub id: ProfileId,
    pub server: ServerId,
    pub version: u64,
    pub record: uuid::Uuid,
    pub bytes: Vec<u8>,
}
impl ProfileService {
    pub(super) async fn append(
        &self,
        entry: NewVersion,
        lease: &SessionLease,
    ) -> Result<(), ProfileError> {
        let number = i64::try_from(entry.version).map_err(|_| ProfileError::Conflict)?;
        let _writer = self.store.catalog_writer.lock().await;
        if !lease.is_live() {
            return Err(ProfileError::Unauthorized);
        }
        let mut transaction = self
            .store
            .pool
            .begin()
            .await
            .map_err(|_| ProfileError::Storage)?;
        sqlx::query("INSERT INTO encrypted_records VALUES(?,'profile',?,?)")
            .bind(entry.record.to_string())
            .bind(number)
            .bind(entry.bytes)
            .execute(&mut *transaction)
            .await
            .map_err(|_| ProfileError::Storage)?;
        sqlx::query("INSERT INTO profile_versions VALUES(?,?,?,?)")
            .bind(entry.id.to_string())
            .bind(number)
            .bind(entry.server.to_string())
            .bind(entry.record.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(super::repository::write_error)?;
        transaction
            .commit()
            .await
            .map_err(|_| ProfileError::Storage)?;
        Ok(())
    }
}
