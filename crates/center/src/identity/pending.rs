use super::{IdentityError, IssuedAgent};
use crate::{auth::SessionLease, storage::CenterStore};
use hmail_cleaner_protocol::AgentId;
use sqlx::Row;
const MAX_PENDING_BYTES: usize = 128 * 1024;
pub(super) struct Pending {
    pub expected_serial: String,
    pub issued: IssuedAgent,
}
pub(super) async fn current(store: &CenterStore, agent: AgentId) -> Result<String, IdentityError> {
    let serial:Option<Option<String>>=sqlx::query_scalar("SELECT CASE WHEN length(serial)<=24 THEN serial ELSE NULL END FROM trust WHERE agent_id=? AND revoked=0").bind(agent.to_string()).fetch_optional(&store.pool).await.map_err(|_|IdentityError)?;
    let serial = serial.flatten().ok_or(IdentityError)?;
    super::serial_bytes(&serial)?;
    Ok(serial)
}
pub(super) async fn load(
    store: &CenterStore,
    agent: AgentId,
) -> Result<Option<Pending>, IdentityError> {
    let row=sqlx::query("SELECT CASE WHEN length(expected_serial)<=24 THEN expected_serial ELSE NULL END AS expected,CASE WHEN length(candidate)<=? THEN candidate ELSE NULL END AS bounded FROM identity_renewals WHERE agent_id=?")
        .bind(MAX_PENDING_BYTES as i64).bind(agent.to_string()).fetch_optional(&store.pool).await.map_err(|_|IdentityError)?;
    row.map(|row| {
        let expected_serial: String = row.try_get("expected").map_err(|_| IdentityError)?;
        super::serial_bytes(&expected_serial)?;
        let bytes: Vec<u8> = row.try_get("bounded").map_err(|_| IdentityError)?;
        let issued: IssuedAgent = serde_json::from_slice(&bytes).map_err(|_| IdentityError)?;
        if issued.serial.is_empty()
            || issued.serial.len() > 17
            || issued.certificate_pem.len() > 65536
            || issued.ca_pem.len() > 65536
        {
            return Err(IdentityError);
        }
        Ok(Pending {
            expected_serial,
            issued,
        })
    })
    .transpose()
}
pub(super) async fn prepare(
    store: CenterStore,
    lease: SessionLease,
    agent: AgentId,
    pending: Pending,
) -> Result<Pending, IdentityError> {
    let writer = store
        .catalog_writer
        .clone()
        .try_lock_owned()
        .map_err(|_| IdentityError)?;
    let bytes = serde_json::to_vec(&pending.issued).map_err(|_| IdentityError)?;
    if bytes.len() > MAX_PENDING_BYTES {
        return Err(IdentityError);
    }
    tokio::spawn(async move {
        let _writer=writer;if !lease.is_live(){return Err(IdentityError);}
        let mut transaction=store.pool.begin().await.map_err(|_|IdentityError)?;
        let row:Option<Option<String>>=sqlx::query_scalar("SELECT CASE WHEN length(serial)<=24 THEN serial ELSE NULL END FROM trust WHERE agent_id=? AND revoked=0").bind(agent.to_string()).fetch_optional(&mut *transaction).await.map_err(|_|IdentityError)?;
        if row.flatten().as_deref()!=Some(pending.expected_serial.as_str()){return Err(IdentityError);}
        sqlx::query("INSERT INTO identity_renewals(agent_id,expected_serial,candidate) VALUES(?,?,?)").bind(agent.to_string()).bind(&pending.expected_serial).bind(bytes).execute(&mut *transaction).await.map_err(|_|IdentityError)?;
        transaction.commit().await.map_err(|_|IdentityError)?;Ok(pending)
    }).await.map_err(|_|IdentityError)?
}
