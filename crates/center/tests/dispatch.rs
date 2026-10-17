pub mod fixtures;
use hmail_cleaner_center::jobs::{JobService, SubmitError};
use hmail_cleaner_protocol::{
    browser::jobs::SubmitRequest, JobId, Operation, RunLimits, Scan, ServicePolicy,
};
#[tokio::test]
async fn lost_delivery_holds_the_global_slot_and_repeated_id_never_resubmits() {
    let fixture = fixtures::center().await;
    let target = fixtures::jobs::register(&fixture, "https://127.0.0.1:9").await;
    let service = JobService::new(fixture.store.clone());
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
    let first = service.submit(&fixture.lease, input.clone()).await.unwrap();
    assert!(first.slot_held);
    assert!(first.snapshot.is_none());
    assert_eq!(
        service
            .submit(&fixture.lease, input.clone())
            .await
            .unwrap()
            .job_id,
        first.job_id
    );
    let mut other = input;
    other.job_id = JobId::new();
    assert!(matches!(
        service.submit(&fixture.lease, other).await,
        Err(SubmitError::Busy)
    ));
    let restarted = JobService::new(fixture.store.clone());
    assert!(
        restarted
            .reconcile(first.job_id, &fixture.lease)
            .await
            .unwrap()
            .slot_held
    );
}
