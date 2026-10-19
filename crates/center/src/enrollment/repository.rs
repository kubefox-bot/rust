use super::{remote::Connector, EnrollmentError, EnrollmentService};
use crate::auth::SessionLease;
use hmail_cleaner_protocol::{
    browser::enrollment::{EnrollmentPhase, EnrollmentView},
    AgentId,
};
use sqlx::Row;
use uuid::Uuid;
impl<C: Connector> EnrollmentService<C> {
    pub async fn get(
        &self,
        id: Uuid,
        lease: &SessionLease,
    ) -> Result<EnrollmentView, EnrollmentError> {
        if !lease.is_live() {
            return Err(EnrollmentError::Unauthorized);
        }
        self.load(id).await
    }
    pub(super) async fn load(&self, id: Uuid) -> Result<EnrollmentView, EnrollmentError> {
        let row=sqlx::query("SELECT CASE WHEN length(details)<=8192 THEN details END AS details,(SELECT enrollment_id=id FROM provisioning_slot WHERE singleton=1) AS held FROM enrollment WHERE id=?").bind(id.to_string()).fetch_optional(&self.store.pool).await.map_err(|_|EnrollmentError::Storage)?.ok_or(EnrollmentError::NotFound)?;
        let bytes: Vec<u8> = row
            .try_get("details")
            .map_err(|_| EnrollmentError::Storage)?;
        let mut view: EnrollmentView =
            serde_json::from_slice(&bytes).map_err(|_| EnrollmentError::Storage)?;
        if view.id != id {
            return Err(EnrollmentError::Storage);
        }
        view.slot_held = row
            .try_get::<Option<bool>, _>("held")
            .map_err(|_| EnrollmentError::Storage)?
            .unwrap_or(false);
        Ok(view)
    }
    pub(super) async fn reserve(
        &self,
        view: &EnrollmentView,
        lease: &SessionLease,
        kind: Kind,
    ) -> Result<(), EnrollmentError> {
        let mut tx = self
            .store
            .pool
            .begin()
            .await
            .map_err(|_| EnrollmentError::Storage)?;
        // Same first writer reservation as job admission, before checking either slot.
        sqlx::query("UPDATE job_slot SET job_id=job_id WHERE singleton=1")
            .execute(&mut *tx)
            .await
            .map_err(|_| EnrollmentError::Storage)?;
        if !lease.is_live() {
            return Err(EnrollmentError::Unauthorized);
        }
        let occupied:bool=sqlx::query_scalar("SELECT (SELECT job_id IS NOT NULL FROM job_slot WHERE singleton=1) OR (SELECT enrollment_id IS NOT NULL FROM provisioning_slot WHERE singleton=1)").fetch_one(&mut *tx).await.map_err(|_|EnrollmentError::Storage)?;
        if occupied {
            return Err(EnrollmentError::Busy);
        }
        let exists:bool=match kind{
            Kind::Install=>sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM servers WHERE id=? OR agent_id=? OR active_profile_id=?) OR EXISTS(SELECT 1 FROM trust WHERE agent_id=?)")
                .bind(view.server_id.to_string()).bind(view.agent_id.to_string()).bind(view.profile_id.to_string()).bind(view.agent_id.to_string()).fetch_one(&mut *tx).await,
            Kind::Update=>sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM servers WHERE id=? AND agent_id=? AND active_profile_id=?) OR EXISTS(SELECT 1 FROM trust WHERE agent_id=? AND revoked=1)")
                .bind(view.server_id.to_string()).bind(view.agent_id.to_string()).bind(view.profile_id.to_string()).bind(view.agent_id.to_string()).fetch_one(&mut *tx).await,
        }.map_err(|_|EnrollmentError::Storage)?;
        if exists {
            return Err(EnrollmentError::Conflict);
        }
        sqlx::query("INSERT INTO enrollment(id,agent_id,phase,host_key,details) VALUES(?,?,?,?,?)")
            .bind(view.id.to_string())
            .bind(view.agent_id.to_string())
            .bind("preparing")
            .bind(&view.fingerprint)
            .bind(serde_json::to_vec(view).map_err(|_| EnrollmentError::Storage)?)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                if error
                    .as_database_error()
                    .is_some_and(|e| e.is_unique_violation())
                {
                    EnrollmentError::Conflict
                } else {
                    EnrollmentError::Storage
                }
            })?;
        sqlx::query("UPDATE provisioning_slot SET enrollment_id=? WHERE singleton=1")
            .bind(view.id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| EnrollmentError::Storage)?;
        tx.commit().await.map_err(|_| EnrollmentError::Storage)
    }
    pub(super) async fn record(
        &self,
        view: &mut EnrollmentView,
        phase: EnrollmentPhase,
    ) -> Result<(), EnrollmentError> {
        let release = matches!(phase, EnrollmentPhase::Ready | EnrollmentPhase::Failed);
        let mut next = view.clone();
        next.phase = phase;
        next.slot_held = !release;
        let mut tx = self
            .store
            .pool
            .begin()
            .await
            .map_err(|_| EnrollmentError::Storage)?;
        sqlx::query("UPDATE job_slot SET job_id=job_id WHERE singleton=1")
            .execute(&mut *tx)
            .await
            .map_err(|_| EnrollmentError::Storage)?;
        let previous: Vec<u8> = sqlx::query_scalar(
            "SELECT CASE WHEN length(details)<=8192 THEN details END FROM enrollment WHERE id=?",
        )
        .bind(view.id.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| EnrollmentError::Storage)?;
        let current: EnrollmentView =
            serde_json::from_slice(&previous).map_err(|_| EnrollmentError::Storage)?;
        if current.phase == EnrollmentPhase::Ready {
            *view = current;
            return Ok(());
        }
        sqlx::query("UPDATE enrollment SET phase=?,details=? WHERE id=?")
            .bind(phase.as_str())
            .bind(serde_json::to_vec(&next).map_err(|_| EnrollmentError::Storage)?)
            .bind(view.id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| EnrollmentError::Storage)?;
        if release {
            sqlx::query("UPDATE provisioning_slot SET enrollment_id=NULL WHERE singleton=1 AND enrollment_id=?").bind(view.id.to_string()).execute(&mut *tx).await.map_err(|_|EnrollmentError::Storage)?;
        }
        tx.commit().await.map_err(|_| EnrollmentError::Storage)?;
        *view = next;
        Ok(())
    }
    pub(super) async fn server_for_agent(
        &self,
        id: AgentId,
    ) -> Result<hmail_cleaner_protocol::ServerId, EnrollmentError> {
        let value: String = sqlx::query_scalar("SELECT id FROM servers WHERE agent_id=?")
            .bind(id.to_string())
            .fetch_optional(&self.store.pool)
            .await
            .map_err(|_| EnrollmentError::Storage)?
            .ok_or(EnrollmentError::NotFound)?;
        serde_json::from_value(serde_json::Value::String(value))
            .map_err(|_| EnrollmentError::Storage)
    }
}
pub(super) enum Kind {
    Install,
    Update,
}
