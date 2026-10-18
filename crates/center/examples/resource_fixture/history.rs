use crate::fixtures::{self, CenterFixture};
use hmail_cleaner_center::profiles::ServerRegistration;
use hmail_cleaner_protocol::{
    browser::jobs::SubmitRequest, JobId, JobPhase, JobSnapshot, JobStage, ObservedMailState,
    Operation, PublicJob, RunLimits, Scan, ServicePolicy, SummaryDto,
};
/// Populate synthetic terminal receipts for the resource workload's history page.
pub async fn seed(center: &CenterFixture, target: &ServerRegistration) {
    const HISTORY_ROWS: usize = 51;
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        center.root.path().join("center.sqlite").display()
    ))
    .await
    .unwrap();
    let epoch = uuid::Uuid::new_v4();
    for _ in 0..HISTORY_ROWS {
        let input = SubmitRequest {
            job_id: JobId::new(),
            server_id: target.server_id,
            profile_version: 2.into(),
            operation: Operation::Preview,
            scan: Scan::Fast,
            resolved_cutoff: fixtures::jobs::cutoff(),
            service_policy: ServicePolicy::RequireStopped,
            limits: RunLimits::default(),
            preview_job_id: None,
            confirmed: false,
        };
        let snapshot = JobSnapshot {
            job_id: input.job_id,
            job: Some(PublicJob {
                agent_id: target.agent_id,
                profile_id: target.profile_id,
                profile_version: input.profile_version,
                operation: input.operation,
                scan: input.scan,
                resolved_cutoff: input.resolved_cutoff,
                service_policy: input.service_policy,
                limits: input.limits,
            }),
            stream_epoch: epoch,
            revision: 2.into(),
            phase: JobPhase::Completed,
            stage: Some(JobStage::Complete),
            summary: SummaryDto::default(),
            mail_state: ObservedMailState::Running,
            initial_mail_state: Some(ObservedMailState::Running),
            error: None,
            observed_at: chrono::Utc::now(),
        };
        sqlx::query("INSERT INTO jobs(id,server_id,profile_id,profile_version,phase,intent,snapshot,revision) VALUES(?,?,?,2,'completed',?,?,2)")
            .bind(input.job_id.to_string()).bind(target.server_id.to_string()).bind(target.profile_id.to_string())
            .bind(serde_json::to_vec(&input).unwrap()).bind(serde_json::to_vec(&snapshot).unwrap()).execute(&pool).await.unwrap();
    }
    pool.close().await;
}
