use super::{JobService, SubmitError};
use crate::auth::SessionLease;
use hmail_cleaner_protocol::{
    browser::jobs::{DispatchState, JobView, SubmitRequest},
    JobId, ProfileId,
};
use sqlx::{Row, Sqlite, Transaction};
pub(crate) const JOB_COLUMNS:&str="SELECT sequence,id,CASE WHEN length(intent)<=8192 THEN intent END AS intent,CASE WHEN length(snapshot)<=65536 THEN snapshot END AS snapshot,revision,(SELECT job_id=id FROM job_slot WHERE singleton=1) AS held FROM jobs";
pub(crate) fn decode(
    row: sqlx::sqlite::SqliteRow,
    epoch: uuid::Uuid,
) -> Result<JobView, SubmitError> {
    let bytes: Vec<u8> = row.try_get("intent")?;
    if bytes.len() > 8192 {
        return Err(SubmitError::Storage);
    }
    let intent: SubmitRequest = serde_json::from_slice(&bytes)?;
    let snapshot: Option<Vec<u8>> = row.try_get("snapshot")?;
    let snapshot = snapshot
        .map(|bytes| serde_json::from_slice(&bytes))
        .transpose()?;
    let id: String = row.try_get("id")?;
    if id != intent.job_id.to_string() {
        return Err(SubmitError::Storage);
    }
    let revision: i64 = row.try_get("revision")?;
    Ok(JobView {
        job_id: intent.job_id,
        intent,
        dispatch: if snapshot.is_some() {
            DispatchState::Acknowledged
        } else {
            DispatchState::Unknown
        },
        snapshot,
        slot_held: row.try_get::<Option<bool>, _>("held")?.unwrap_or(false),
        stream_epoch: epoch,
        revision: u64::try_from(revision)
            .map_err(|_| SubmitError::Storage)?
            .into(),
    })
}
impl JobService {
    pub async fn get(&self, id: JobId, lease: &SessionLease) -> Result<JobView, SubmitError> {
        if !lease.is_live() {
            return Err(SubmitError::Unauthorized);
        }
        self.load(id).await
    }
    pub(super) async fn load(&self, id: JobId) -> Result<JobView, SubmitError> {
        let row = sqlx::query(&format!("{JOB_COLUMNS} WHERE id=?"))
            .bind(id.to_string())
            .fetch_optional(&self.store.pool)
            .await?
            .ok_or(SubmitError::NotFound)?;
        decode(row, self.epoch)
    }
    pub(super) async fn writer(&self) -> Result<Transaction<'_, Sqlite>, SubmitError> {
        let mut tx = self.store.pool.begin().await?;
        sqlx::query("UPDATE job_slot SET job_id=job_id WHERE singleton=1")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
    /// The durable slot is reserved before any network operation; a lost HTTP waiter cannot undo it.
    pub(super) async fn reserve(
        &self,
        input: &SubmitRequest,
        profile: ProfileId,
        lease: &SessionLease,
    ) -> Result<bool, SubmitError> {
        let mut tx = self.writer().await?;
        if !lease.is_live() {
            return Err(SubmitError::Unauthorized);
        }
        let existing: Option<Vec<u8>> = sqlx::query_scalar("SELECT intent FROM jobs WHERE id=?")
            .bind(input.job_id.to_string())
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(bytes) = existing {
            let previous: SubmitRequest = serde_json::from_slice(&bytes)?;
            if previous != *input {
                return Err(SubmitError::Conflict);
            }
            return Ok(false);
        }
        let occupied: bool =
            sqlx::query_scalar("SELECT job_id IS NOT NULL FROM job_slot WHERE singleton=1")
                .fetch_one(&mut *tx)
                .await?;
        if occupied {
            return Err(SubmitError::Busy);
        }
        let provisioning: bool = sqlx::query_scalar(
            "SELECT enrollment_id IS NOT NULL FROM provisioning_slot WHERE singleton=1",
        )
        .fetch_one(&mut *tx)
        .await?;
        if provisioning {
            return Err(SubmitError::Busy);
        }
        let latest: i64 = sqlx::query_scalar(
            "SELECT max(version) FROM profile_versions WHERE id=? AND server_id=?",
        )
        .bind(profile.to_string())
        .bind(input.server_id.to_string())
        .fetch_one(&mut *tx)
        .await?;
        if u64::try_from(latest).ok() != Some(input.profile_version.get()) {
            return Err(SubmitError::Conflict);
        }
        sqlx::query("INSERT INTO jobs(id,server_id,profile_id,profile_version,phase,intent) VALUES(?,?,?,?,?,?)")
            .bind(input.job_id.to_string()).bind(input.server_id.to_string()).bind(profile.to_string()).bind(i64::try_from(input.profile_version.get()).map_err(|_|SubmitError::Invalid)?).bind("unknown").bind(serde_json::to_vec(input)?).execute(&mut *tx).await?;
        sqlx::query("UPDATE job_slot SET job_id=? WHERE singleton=1")
            .bind(input.job_id.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }
    pub(super) async fn profile_id(&self, id: JobId) -> Result<ProfileId, SubmitError> {
        let value: String = sqlx::query_scalar("SELECT profile_id FROM jobs WHERE id=?")
            .bind(id.to_string())
            .fetch_one(&self.store.pool)
            .await?;
        serde_json::from_value(serde_json::Value::String(value)).map_err(|_| SubmitError::Storage)
    }
}
