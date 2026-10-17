use super::{JobRepository, JobStoreError};
impl JobRepository {
    pub async fn begin_maintenance(&self) -> Result<bool, JobStoreError> {
        let mut transaction = self.writer().await?;
        let active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE phase IN ('accepted','running','cancelling','restoring')").fetch_one(&mut *transaction).await?;
        if active != 0 {
            return Ok(false);
        }
        sqlx::query("UPDATE coordination SET maintenance = 1 WHERE id = 1")
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(true)
    }
    pub async fn in_maintenance(&self) -> Result<bool, JobStoreError> {
        let flag: i64 = sqlx::query_scalar("SELECT maintenance FROM coordination WHERE id = 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(flag != 0)
    }
    pub async fn end_maintenance(&self) -> Result<(), JobStoreError> {
        let mut transaction = self.writer().await?;
        sqlx::query("UPDATE coordination SET maintenance = 0 WHERE id = 1")
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}
