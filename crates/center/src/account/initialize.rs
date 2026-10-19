use crate::{
    storage::CenterStore,
    vault::{self, SecretText, SystemEntropy},
};
pub struct InitInput {
    pub login: String,
}
pub struct InitReceipt {
    pub vault_id: uuid::Uuid,
}
pub trait SecretPrompt {
    fn password(&mut self) -> Result<SecretText, String>;
    fn confirm_recovery(&mut self, recovery: &SecretText) -> Result<bool, String>;
}
#[derive(Debug, thiserror::Error)]
#[error("center initialization rejected; no account was replaced")]
pub struct InitError;
pub async fn initialize(
    store: &CenterStore,
    input: &InitInput,
    prompt: &mut impl SecretPrompt,
) -> Result<InitReceipt, InitError> {
    if !super::login::valid(&input.login) {
        return Err(InitError);
    }
    if store.account().await.map_err(|_| InitError)?.is_some() {
        return Err(InitError);
    }
    let password = prompt.password().map_err(|_| InitError)?;
    let prepared = store
        .kdf
        .try_run(move || vault::initialize(&password, &SystemEntropy))
        .await
        .map_err(|_| InitError)?;
    if !prompt
        .confirm_recovery(&prepared.recovery.expose())
        .map_err(|_| InitError)?
    {
        return Err(InitError);
    }
    let encoded = prepared.material.encode().map_err(|_| InitError)?;
    let vault_id = prepared.material.vault_id();
    let config = crate::config::CenterConfig::load(&crate::config::CenterArgs {
        data_dir: store.directory().to_path_buf(),
    })
    .map_err(|_| InitError)?;
    let identity = crate::identity::prepare(&prepared.material, &prepared.dek, &config.origin)
        .map_err(|_| InitError)?;
    let mut transaction = store.pool.begin().await.map_err(|_| InitError)?;
    sqlx::query("INSERT INTO account(singleton,login,revision,material) VALUES(1,?,1,?)")
        .bind(&input.login)
        .bind(encoded)
        .execute(&mut *transaction)
        .await
        .map_err(|_| InitError)?;
    sqlx::query("INSERT INTO identity VALUES(1,?)")
        .bind(vault_id.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(|_| InitError)?;
    sqlx::query("INSERT INTO encrypted_records(id,kind,version,envelope) VALUES(?,'ca_key',1,?)")
        .bind(vault_id.to_string())
        .bind(identity.ca.encode().map_err(|_| InitError)?)
        .execute(&mut *transaction)
        .await
        .map_err(|_| InitError)?;
    identity.persist(store.directory()).map_err(|_| InitError)?;
    sqlx::query(
        "INSERT INTO encrypted_records(id,kind,version,envelope) VALUES(?,'center_client_key',1,?)",
    )
    .bind(prepared.material.account_id().to_string())
    .bind(identity.client.encode().map_err(|_| InitError)?)
    .execute(&mut *transaction)
    .await
    .map_err(|_| InitError)?;
    transaction.commit().await.map_err(|_| InitError)?;
    Ok(InitReceipt { vault_id })
}
