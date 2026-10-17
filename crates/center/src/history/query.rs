use crate::{
    auth::SessionLease,
    jobs::{JobService, SubmitError},
};
use hmail_cleaner_protocol::{browser::jobs::HistoryPage, Counter, ServerId};
use sqlx::Row;
pub const DEFAULT_PAGE_SIZE: u16 = 50;
pub const MAX_PAGE_SIZE: u16 = 100;
impl JobService {
    pub async fn history(
        &self,
        server: ServerId,
        cursor: Option<Counter>,
        limit: u16,
        lease: &SessionLease,
    ) -> Result<HistoryPage, SubmitError> {
        if !lease.is_live() {
            return Err(SubmitError::Unauthorized);
        }
        if !(1..=MAX_PAGE_SIZE).contains(&limit) {
            return Err(SubmitError::Invalid);
        }
        let before = match cursor {
            Some(value) => i64::try_from(value.get()).map_err(|_| SubmitError::Invalid)?,
            None => i64::MAX,
        };
        let rows = sqlx::query(&format!(
            "{} WHERE server_id=? AND sequence<? ORDER BY sequence DESC LIMIT ?",
            crate::jobs::repository::JOB_COLUMNS
        ))
        .bind(server.to_string())
        .bind(before)
        .bind(i64::from(limit) + 1)
        .fetch_all(&self.store.pool)
        .await?;
        let more = rows.len() > usize::from(limit);
        let mut jobs = Vec::with_capacity(usize::from(limit));
        let mut last = None;
        for row in rows.into_iter().take(usize::from(limit)) {
            let sequence: i64 = row.try_get("sequence")?;
            last = Some(
                u64::try_from(sequence)
                    .map_err(|_| SubmitError::Storage)?
                    .into(),
            );
            jobs.push(crate::jobs::repository::decode(row, self.epoch)?);
        }
        Ok(HistoryPage {
            jobs,
            next_cursor: if more { last } else { None },
        })
    }
}
