use super::{material::ClientSecret, IdentityError};
use crate::{
    auth::SessionLease,
    storage::CenterStore,
    vault::{self, AccountMaterial, RecordKind},
};
pub(super) struct StoredClient {
    pub material: AccountMaterial,
    pub secret: ClientSecret,
    pub version: u64,
}
pub(super) async fn load(
    store: &CenterStore,
    lease: &SessionLease,
) -> Result<StoredClient, IdentityError> {
    let account = store
        .account()
        .await
        .map_err(|_| IdentityError)?
        .ok_or(IdentityError)?;
    if !lease.is_live() {
        return Err(IdentityError);
    }
    let version: i64 = sqlx::query_scalar(
        "SELECT version FROM encrypted_records WHERE id=? AND kind='center_client_key'",
    )
    .bind(account.material.account_id().to_string())
    .fetch_one(&store.pool)
    .await
    .map_err(|_| IdentityError)?;
    let header = super::policy::header(
        &account.material,
        RecordKind::CenterClientKey,
        account.material.account_id(),
        u64::try_from(version).map_err(|_| IdentityError)?,
    )?;
    let record = store
        .encrypted(account.material.account_id(), &header)
        .await
        .map_err(|_| IdentityError)?
        .ok_or(IdentityError)?;
    let plaintext = lease
        .with_dek(|dek| vault::open_record(dek, &header, &record))
        .map_err(|_| IdentityError)?
        .map_err(|_| IdentityError)?;
    let secret: ClientSecret = serde_json::from_slice(&plaintext).map_err(|_| IdentityError)?;
    Ok(StoredClient {
        material: account.material,
        secret,
        version: u64::try_from(version).map_err(|_| IdentityError)?,
    })
}
