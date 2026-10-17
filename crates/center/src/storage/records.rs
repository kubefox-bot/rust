use super::{CenterStore, StorageError};
use crate::vault::{EncryptedRecord, EnvelopeHeader, MAX_ENCRYPTED_RECORD_BYTES};
use sqlx::Row;
impl CenterStore {
    pub async fn insert_encrypted(
        &self,
        id: uuid::Uuid,
        kind: &str,
        version: i64,
        record: &EncryptedRecord,
    ) -> Result<(), StorageError> {
        let header = record.header();
        let stored_kind = match header.kind() {
            crate::vault::RecordKind::Profile => "profile",
            crate::vault::RecordKind::CaKey => "ca_key",
            crate::vault::RecordKind::CenterClientKey => "center_client_key",
            crate::vault::RecordKind::PasswordWrap | crate::vault::RecordKind::RecoveryWrap => {
                return Err(StorageError::Account)
            }
        };
        if id != header.record_id()
            || u64::try_from(version).ok() != Some(header.record_version())
            || kind != stored_kind
        {
            return Err(StorageError::Account);
        }
        let bytes = record.encode().map_err(|_| StorageError::Account)?;
        sqlx::query("INSERT INTO encrypted_records(id,kind,version,envelope) VALUES(?,?,?,?)")
            .bind(id.to_string())
            .bind(kind)
            .bind(version)
            .bind(bytes)
            .execute(&self.pool)
            .await
            .map_err(|_| StorageError::Database)?;
        Ok(())
    }
    pub async fn encrypted(
        &self,
        id: uuid::Uuid,
        expected: &EnvelopeHeader,
    ) -> Result<Option<EncryptedRecord>, StorageError> {
        let row=sqlx::query("SELECT CASE WHEN length(envelope)<=? THEN envelope ELSE NULL END AS bounded FROM encrypted_records WHERE id=?")
            .bind(MAX_ENCRYPTED_RECORD_BYTES as i64).bind(id.to_string()).fetch_optional(&self.pool).await.map_err(|_|StorageError::Database)?;
        row.map(|row| {
            let bytes: Option<Vec<u8>> =
                row.try_get("bounded").map_err(|_| StorageError::Account)?;
            let record = EncryptedRecord::decode(&bytes.ok_or(StorageError::Account)?)
                .map_err(|_| StorageError::Account)?;
            if record.header() != expected {
                return Err(StorageError::Account);
            }
            Ok(record)
        })
        .transpose()
    }
}
