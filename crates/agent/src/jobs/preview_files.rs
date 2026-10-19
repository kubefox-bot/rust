use super::{JobRepository, JobStoreError};
use chrono::{DateTime, SecondsFormat, Utc};
use hmail_cleaner_protocol::{
    JobId, JobSnapshot, Operation, PreviewFile, PreviewFileSort, PreviewFilesPage,
    PreviewFilesQuery, SortDirection,
};
use sqlx::{QueryBuilder, Row};

// Seven bindings per row; stay below even SQLite's older 999-parameter limit.
const INSERT_ROWS: usize = 128;

impl JobRepository {
    /// Owns persistence of one bounded scan batch before traversal may continue.
    pub async fn append_preview_files(
        &self,
        id: JobId,
        files: &[PreviewFile],
    ) -> Result<JobSnapshot, JobStoreError> {
        for file in files {
            file.validate().map_err(|_| JobStoreError::InvalidReceipt)?;
        }
        let mut transaction = self.writer().await?;
        let row = sqlx::query("SELECT job_id, phase, snapshot FROM jobs WHERE job_id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(JobStoreError::NotFound)?;
        let mut snapshot = self.decode(&row, id)?;
        if !snapshot.phase.is_active()
            || snapshot
                .job
                .as_ref()
                .is_none_or(|job| job.operation != Operation::Preview)
        {
            return Err(JobStoreError::InvalidTransition);
        }
        require_capture(&mut transaction, id).await?;
        let job_id = id.to_string();
        for chunk in files.chunks(INSERT_ROWS) {
            let mut query = QueryBuilder::new("INSERT INTO preview_files(job_id,id,path,search_path,size_bytes,created_at,orphan) ");
            query.push_values(chunk, |mut row, file| {
                row.push_bind(&job_id)
                    .push_bind(&file.id)
                    .push_bind(&file.path)
                    .push_bind(file.path.to_lowercase())
                    .push_bind(file.size_bytes.get().to_string())
                    .push_bind(file.created_at.to_rfc3339_opts(SecondsFormat::Nanos, true))
                    .push_bind(file.orphan);
            });
            query.push(" ON CONFLICT(job_id,id) DO NOTHING");
            query
                .build()
                .persistent(false)
                .execute(&mut *transaction)
                .await?;
        }
        self.bump(&mut snapshot)?;
        self.save(&mut transaction, &snapshot).await?;
        transaction.commit().await?;
        Ok(snapshot)
    }

    pub async fn preview_files(
        &self,
        id: JobId,
        query: &PreviewFilesQuery,
    ) -> Result<PreviewFilesPage, JobStoreError> {
        query
            .validate()
            .map_err(|_| JobStoreError::InvalidReceipt)?;
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query("SELECT job_id, phase, snapshot FROM jobs WHERE job_id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(JobStoreError::NotFound)?;
        let snapshot = self.decode(&row, id)?;
        if snapshot
            .job
            .as_ref()
            .is_none_or(|job| job.operation != Operation::Preview)
        {
            return Err(JobStoreError::InvalidTransition);
        }
        require_capture(&mut transaction, id).await?;
        let search = query.search.to_lowercase();
        let from = query
            .created_from
            .map(|date| date.to_rfc3339_opts(SecondsFormat::Nanos, true));
        let to = query
            .created_to
            .map(|date| date.to_rfc3339_opts(SecondsFormat::Nanos, true));
        let predicate = "job_id = ?1 AND instr(search_path, ?2) > 0 AND (?3 IS NULL OR created_at >= ?3) AND (?4 IS NULL OR created_at <= ?4) AND (?5 IS NULL OR orphan = ?5)";
        let total: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM preview_files WHERE {predicate}"
        ))
        .bind(id.to_string())
        .bind(&search)
        .bind(&from)
        .bind(&to)
        .bind(query.orphan)
        .fetch_one(&mut *transaction)
        .await?;
        let direction = match query.direction {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        };
        // Only closed-set SQL fragments are interpolated; all caller input is bound.
        let order = match query.sort {
            PreviewFileSort::Path => format!("path {direction}"),
            PreviewFileSort::Size => {
                format!("length(size_bytes) {direction}, size_bytes {direction}")
            }
            PreviewFileSort::CreatedAt => format!("created_at {direction}"),
        };
        let sql = format!("SELECT id,path,size_bytes,created_at,orphan FROM preview_files WHERE {predicate} ORDER BY {order}, id ASC LIMIT ?6 OFFSET ?7");
        let rows = sqlx::query(&sql)
            .bind(id.to_string())
            .bind(search)
            .bind(&from)
            .bind(&to)
            .bind(query.orphan)
            .bind(i64::from(query.limit))
            .bind(query.offset as i64)
            .fetch_all(&mut *transaction)
            .await?;
        let rows = rows
            .into_iter()
            .map(|row| {
                let size: String = row.try_get("size_bytes")?;
                let created: String = row.try_get("created_at")?;
                Ok(PreviewFile {
                    id: row.try_get("id")?,
                    path: row.try_get("path")?,
                    size_bytes: size
                        .parse::<u64>()
                        .map_err(|_| JobStoreError::InvalidReceipt)?
                        .into(),
                    created_at: DateTime::parse_from_rfc3339(&created)
                        .map_err(|_| JobStoreError::InvalidReceipt)?
                        .with_timezone(&Utc),
                    orphan: row.try_get("orphan")?,
                })
            })
            .collect::<Result<Vec<_>, JobStoreError>>()?;
        transaction.commit().await?;
        Ok(PreviewFilesPage {
            job_id: id,
            rows,
            total: (total as u64).into(),
            revision: snapshot.revision,
            complete: !snapshot.phase.is_active(),
        })
    }
}

async fn require_capture(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: JobId,
) -> Result<(), JobStoreError> {
    let captured: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM preview_catalogs WHERE job_id = ?)")
            .bind(id.to_string())
            .fetch_one(&mut **transaction)
            .await?;
    if !captured {
        return Err(JobStoreError::PreviewUnavailable);
    }
    Ok(())
}
