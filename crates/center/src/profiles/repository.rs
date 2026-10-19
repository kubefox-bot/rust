use super::{
    types::{StoredProfile, TechnicalProfile},
    ProfileError, ProfileService,
};
use crate::{
    auth::SessionLease,
    vault::{self, EnvelopeHeader, RecordIdentity, RecordKind},
};
use hmail_cleaner_protocol::ProfileId;
use sqlx::Row;
impl ProfileService {
    pub(super) async fn load(
        &self,
        id: ProfileId,
        version: Option<u64>,
        lease: &SessionLease,
    ) -> Result<StoredProfile, ProfileError> {
        let account = self
            .store
            .account()
            .await
            .map_err(|_| ProfileError::Storage)?
            .ok_or(ProfileError::Storage)?;
        let version = version
            .map(i64::try_from)
            .transpose()
            .map_err(|_| ProfileError::Invalid)?;
        let row=sqlx::query("SELECT version,record_id,server_id FROM profile_versions WHERE id=? AND (? IS NULL OR version=?) ORDER BY version DESC LIMIT 1")
            .bind(id.to_string()).bind(version).bind(version).fetch_optional(&self.store.pool).await.map_err(|_|ProfileError::Storage)?.ok_or(ProfileError::NotFound)?;
        let number: i64 = row.try_get("version").map_err(|_| ProfileError::Storage)?;
        let record: String = row
            .try_get("record_id")
            .map_err(|_| ProfileError::Storage)?;
        let server: String = row
            .try_get("server_id")
            .map_err(|_| ProfileError::Storage)?;
        let record_id = uuid::Uuid::parse_str(&record).map_err(|_| ProfileError::Storage)?;
        let version = u64::try_from(number).map_err(|_| ProfileError::Storage)?;
        if record_id.to_string() != record || version == 0 {
            return Err(ProfileError::Storage);
        }
        let header = EnvelopeHeader::record(RecordIdentity {
            vault_id: account.material.vault_id(),
            kind: RecordKind::Profile,
            record_id,
            record_version: version,
            key_id: account.material.dek_id(),
            key_generation: 1,
        })
        .map_err(|_| ProfileError::Storage)?;
        let encrypted = self
            .store
            .encrypted(record_id, &header)
            .await
            .map_err(|_| ProfileError::Storage)?
            .ok_or(ProfileError::Storage)?;
        let plaintext = lease
            .with_dek(|dek| vault::open_record(dek, &header, &encrypted))
            .map_err(|_| ProfileError::Unauthorized)?
            .map_err(|_| ProfileError::Storage)?;
        let technical: TechnicalProfile =
            serde_json::from_slice(&plaintext).map_err(|_| ProfileError::Storage)?;
        if technical.profile_id != id
            || technical.version != version
            || technical.server_id.to_string() != server
            || technical.credentials.center_id != account.material.vault_id()
        {
            return Err(ProfileError::Storage);
        }
        Ok(StoredProfile {
            technical,
            material: account.material,
        })
    }
    pub(super) fn encrypt(
        profile: &TechnicalProfile,
        material: &vault::AccountMaterial,
        lease: &SessionLease,
    ) -> Result<(uuid::Uuid, Vec<u8>), ProfileError> {
        let record_id = uuid::Uuid::new_v4();
        let header = EnvelopeHeader::record(RecordIdentity {
            vault_id: material.vault_id(),
            kind: RecordKind::Profile,
            record_id,
            record_version: profile.version,
            key_id: material.dek_id(),
            key_generation: 1,
        })
        .map_err(|_| ProfileError::Invalid)?;
        let plaintext = zeroize::Zeroizing::new(
            serde_json::to_vec(profile).map_err(|_| ProfileError::Invalid)?,
        );
        let record = lease
            .with_dek(|dek| vault::seal_record(dek, &header, &plaintext))
            .map_err(|_| ProfileError::Unauthorized)?
            .map_err(|_| ProfileError::Invalid)?;
        Ok((
            record_id,
            record.encode().map_err(|_| ProfileError::Invalid)?,
        ))
    }
}
pub(super) fn write_error(error: sqlx::Error) -> ProfileError {
    match error {
        sqlx::Error::Database(error) if error.is_unique_violation() => ProfileError::Conflict,
        _ => ProfileError::Storage,
    }
}
