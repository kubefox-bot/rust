const RECENT_ENROLLMENTS: i64 = 50;
use super::{remote::Connector, EnrollmentService};
impl<C: Connector> EnrollmentService<C> {
    pub async fn recent(
        &self,
        lease: &crate::auth::SessionLease,
    ) -> Result<
        Vec<hmail_cleaner_protocol::browser::enrollment::EnrollmentView>,
        super::EnrollmentError,
    > {
        super::validation::authorize(lease)?;
        let ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM enrollment ORDER BY rowid DESC LIMIT ?")
                .bind(RECENT_ENROLLMENTS)
                .fetch_all(&self.store.pool)
                .await
                .map_err(|_| super::EnrollmentError::Storage)?;
        let mut views = Vec::with_capacity(ids.len());
        for id in ids {
            let id = uuid::Uuid::parse_str(&id).map_err(|_| super::EnrollmentError::Storage)?;
            views.push(self.get(id, lease).await?);
        }
        Ok(views)
    }
}
