use super::{IdentityError, IssuedAgent};
use crate::{auth::SessionLease, storage::CenterStore};
use hmail_cleaner_protocol::AgentId;
pub(super) enum Activation {
    Enrollment,
    Renewal { expected_serial: String },
}
pub(super) async fn activate(
    store: CenterStore,
    lease: SessionLease,
    agent: AgentId,
    issued: IssuedAgent,
    policy: Activation,
) -> Result<IssuedAgent, IdentityError> {
    let writer = store
        .catalog_writer
        .clone()
        .try_lock_owned()
        .map_err(|_| IdentityError)?;
    let maintenance = store
        .maintenance()
        .continuation()
        .map_err(|_| IdentityError)?;
    tokio::spawn(async move {
        let _maintenance = maintenance;
        let _writer=writer;
        if !lease.is_live(){return Err(IdentityError);}
        let mut transaction=store.pool.begin().await.map_err(|_|IdentityError)?;
        let previous:Option<String>=sqlx::query_scalar("SELECT serial FROM trust WHERE agent_id=?").bind(agent.to_string()).fetch_optional(&mut *transaction).await.map_err(|_|IdentityError)?;
        let old=previous.as_ref().map(|value|super::serial_bytes(value)).transpose()?;
        match policy {
            Activation::Enrollment=>(),
            Activation::Renewal{expected_serial}=>{
                let eligible:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM trust WHERE agent_id=? AND serial=? AND revoked=0)").bind(agent.to_string()).bind(expected_serial).fetch_one(&mut *transaction).await.map_err(|_|IdentityError)?;
                if !eligible{return Err(IdentityError);}
            }
        }

        if old.as_ref()==Some(&issued.serial){return Err(IdentityError);}
        if let Some(previous)=&previous {sqlx::query("INSERT OR IGNORE INTO revoked_serials(serial) VALUES(?)").bind(previous).execute(&mut *transaction).await.map_err(|_|IdentityError)?;}
        sqlx::query("INSERT INTO trust(agent_id,serial,revoked,certificate_pem,expires_at) VALUES(?,?,0,?,?) ON CONFLICT(agent_id) DO UPDATE SET serial=excluded.serial,revoked=0,certificate_pem=excluded.certificate_pem,expires_at=excluded.expires_at")
            .bind(agent.to_string()).bind(super::serial_text(&issued.serial)).bind(&issued.certificate_pem).bind(issued.expires_at_unix).execute(&mut *transaction).await.map_err(|_|IdentityError)?;
        sqlx::query("DELETE FROM identity_renewals WHERE agent_id=?").bind(agent.to_string()).execute(&mut *transaction).await.map_err(|_|IdentityError)?;
        transaction.commit().await.map_err(|_|IdentityError)?;
        if let Some(old)=old {store.revoked.revoke(old);}
        Ok(issued)
    }).await.map_err(|_|IdentityError)?
}
pub(super) async fn revoke(
    store: CenterStore,
    lease: SessionLease,
    agent: AgentId,
) -> Result<(), IdentityError> {
    let writer = store
        .catalog_writer
        .clone()
        .try_lock_owned()
        .map_err(|_| IdentityError)?;
    let maintenance = store
        .maintenance()
        .continuation()
        .map_err(|_| IdentityError)?;
    tokio::spawn(async move {
        let _maintenance = maintenance;
        let _writer = writer;
        if !lease.is_live() {
            return Err(IdentityError);
        }
        let mut transaction = store.pool.begin().await.map_err(|_| IdentityError)?;
        let serial: String = sqlx::query_scalar("SELECT serial FROM trust WHERE agent_id=?")
            .bind(agent.to_string())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|_| IdentityError)?
            .ok_or(IdentityError)?;
        let bytes = super::serial_bytes(&serial)?;
        sqlx::query("INSERT OR IGNORE INTO revoked_serials(serial) VALUES(?)")
            .bind(serial)
            .execute(&mut *transaction)
            .await
            .map_err(|_| IdentityError)?;
        sqlx::query("UPDATE trust SET revoked=1 WHERE agent_id=?")
            .bind(agent.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(|_| IdentityError)?;
        transaction.commit().await.map_err(|_| IdentityError)?;
        store.revoked.revoke(bytes);
        Ok(())
    })
    .await
    .map_err(|_| IdentityError)?
}
