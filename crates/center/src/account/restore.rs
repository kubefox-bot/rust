use super::{AccountError, SecretPrompt};
use crate::{
    storage::CenterStore,
    vault::{self, RecoveryKey, SystemEntropy},
};
pub struct RecoveryReceipt {
    pub revision: i64,
}
/// The CLI must hold the exclusive data-directory lease before prompting.
pub async fn restore(
    store: &CenterStore,
    recovery: &RecoveryKey,
    prompt: &mut impl SecretPrompt,
) -> Result<RecoveryReceipt, AccountError> {
    let account = store
        .account()
        .await
        .map_err(|_| AccountError::Unavailable)?
        .ok_or(AccountError::Invalid)?;
    let revision = account.revision;
    let key = RecoveryKey::parse(&recovery.expose()).map_err(|_| AccountError::Invalid)?;
    let password = prompt.password().map_err(|_| AccountError::Invalid)?;
    let (material, new_key) = store
        .kdf
        .try_run(move || {
            let dek = vault::open_recovery(&account.material, &key)?;
            let password_material =
                vault::replace_password(&account.material, &dek, &password, &SystemEntropy)?;
            let new_key = RecoveryKey::generate(&SystemEntropy)?;
            Ok((
                vault::replace_recovery(&password_material, &dek, &new_key, &SystemEntropy)?,
                new_key,
            ))
        })
        .await
        .map_err(AccountError::from_kdf)?;
    if !prompt
        .confirm_recovery(&new_key.expose())
        .map_err(|_| AccountError::Invalid)?
    {
        return Err(AccountError::Invalid);
    }
    let encoded = material.encode().map_err(|_| AccountError::Unavailable)?;
    let next = revision.checked_add(1).ok_or(AccountError::Unavailable)?;
    let owned_store = store.clone();
    tokio::spawn(async move {
        let mut transaction = owned_store
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
        Ok(RecoveryReceipt { revision: next })
    })
    .await
    .map_err(|_| AccountError::Unavailable)?
}
