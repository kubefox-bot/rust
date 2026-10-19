use crate::fixtures::{self, CenterFixture};
use hmail_cleaner_center::profiles::{ProfileService, ServerRegistration};
use hmail_cleaner_protocol::{
    browser::jobs::SubmitRequest, AgentId, JobId, JobPhase, JobSnapshot, JobStage,
    ObservedMailState, Operation, ProfileId, PublicJob, RunLimits, Scan, ServerId, ServicePolicy,
    SummaryDto,
};
pub async fn unconfigured(
    center: &CenterFixture,
    id: AgentId,
    endpoint: &str,
) -> ServerRegistration {
    let target = ServerRegistration {
        server_id: ServerId::new(),
        profile_id: ProfileId::new(),
        agent_id: id,
        name: "Mail Two".into(),
        endpoint: endpoint.into(),
    };
    ProfileService::new(center.store.clone())
        .register(
            ServerRegistration {
                server_id: target.server_id,
                profile_id: target.profile_id,
                agent_id: id,
                name: target.name.clone(),
                endpoint: target.endpoint.clone(),
            },
            &center.lease,
        )
        .await
        .unwrap();
    target
}
/// Synthetic terminal receipts for pagination only; no simulated mail operation is claimed.
pub async fn history(center: &CenterFixture, target: &ServerRegistration, count: usize) {
    const MAX_FIXTURE_HISTORY: usize = 51;
    assert!(count <= MAX_FIXTURE_HISTORY);
    if count == 0 {
        return;
    }
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        center.root.path().join("center.sqlite").display()
    ))
    .await
    .unwrap();
    let epoch = uuid::Uuid::new_v4();
    for _ in 0..count {
        let input = SubmitRequest {
            job_id: JobId::new(),
            server_id: target.server_id,
            profile_version: 2.into(),
            operation: Operation::Preview,
            scan: Scan::Fast,
            resolved_cutoff: fixtures::jobs::cutoff(),
            service_policy: ServicePolicy::Manage,
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
                service_policy: ServicePolicy::RequireStopped,
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
            .bind(input.job_id.to_string()).bind(target.server_id.to_string())
            .bind(target.profile_id.to_string()).bind(serde_json::to_vec(&input).unwrap())
            .bind(serde_json::to_vec(&snapshot).unwrap()).execute(&pool).await.unwrap();
    }
    pool.close().await;
}
