use crate::{
    storage::{CenterStore, StorageError},
    vault::{AccountMaterial, MAX_ACCOUNT_MATERIAL_BYTES},
};
use sqlx::Row;
pub struct Account {
    pub login: String,
    pub revision: i64,
    pub material: AccountMaterial,
}
impl CenterStore {
    pub async fn account(&self) -> Result<Option<Account>, StorageError> {
        let row = sqlx::query("SELECT login,revision,length(material) AS size,CASE WHEN length(material)<=? THEN material ELSE NULL END AS bounded FROM account WHERE singleton=1")
            .bind(MAX_ACCOUNT_MATERIAL_BYTES as i64).fetch_optional(&self.pool).await.map_err(|_| StorageError::Database)?;
        row.map(|row| {
            let bytes: Option<Vec<u8>> =
                row.try_get("bounded").map_err(|_| StorageError::Account)?;
            let material = AccountMaterial::decode(&bytes.ok_or(StorageError::Account)?)
                .map_err(|_| StorageError::Account)?;
            Ok(Account {
                login: row.try_get("login").map_err(|_| StorageError::Account)?,
                revision: row.try_get("revision").map_err(|_| StorageError::Account)?,
                material,
            })
        })
        .transpose()
    }
}
