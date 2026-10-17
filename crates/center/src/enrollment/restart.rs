use super::{remote::Connector, EnrollmentService};
impl<C: Connector> EnrollmentService<C> {
    /// Called only by the exclusive boot owner, before exposing HTTP. No remote operations.
    pub async fn recover_interrupted(&self) -> Result<(), super::EnrollmentError> {
        let id: Option<String> =
            sqlx::query_scalar("SELECT enrollment_id FROM provisioning_slot WHERE singleton=1")
                .fetch_one(&self.store.pool)
                .await
                .map_err(|_| super::EnrollmentError::Storage)?;
        if let Some(id) = id {
            let id = uuid::Uuid::parse_str(&id).map_err(|_| super::EnrollmentError::Storage)?;
            let mut view = self.load(id).await?;
            use hmail_cleaner_protocol::browser::enrollment::EnrollmentPhase;
            if matches!(view.phase, EnrollmentPhase::Ready | EnrollmentPhase::Failed) {
                return Err(super::EnrollmentError::Storage);
            }
            self.record(&mut view, EnrollmentPhase::Unknown).await?;
        }
        Ok(())
    }
}
