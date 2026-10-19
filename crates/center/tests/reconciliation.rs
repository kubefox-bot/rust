pub mod fixtures;
use hmail_cleaner_center::jobs::{JobService, SubmitError};
use hmail_cleaner_protocol::{
    browser::jobs::SubmitRequest, JobId, JobPhase, ObservedMailState, Operation, RunLimits, Scan,
    ServicePolicy,
};
fn request(server_id: hmail_cleaner_protocol::ServerId) -> SubmitRequest {
    SubmitRequest {
        job_id: JobId::new(),
        server_id,
        profile_version: 2.into(),
        operation: Operation::Preview,
        scan: Scan::Fast,
        resolved_cutoff: fixtures::jobs::cutoff(),
        service_policy: ServicePolicy::Manage,
        limits: RunLimits::default(),
        preview_job_id: None,
        confirmed: false,
    }
}
#[tokio::test]
async fn lost_accept_404_restart_and_cancel_never_replay_a_command() {
    let center = fixtures::center().await;
    let (agent, a) = fixtures::job_agent::start(&center).await;
    let (other, b) = fixtures::job_agent::start(&center).await;
    let target = fixtures::jobs::configured(
        &center,
        agent.credentials.agent_id,
        agent.credentials.endpoint.as_str(),
    )
    .await;
    let second = fixtures::jobs::configured(
        &center,
        other.credentials.agent_id,
        other.credentials.endpoint.as_str(),
    )
    .await;
    let service = JobService::new(center.store.clone());
    a.lose_response(true);
    a.hide_lookup(true);
    let input = request(target.server_id);
    let id = input.job_id;
    assert!(service
        .submit(&center.lease, input.clone())
        .await
        .unwrap()
        .snapshot
        .is_none());
    assert_eq!(a.puts(), 1);
    assert_eq!(b.puts(), 0);
    assert!(
        service
            .reconcile(id, &center.lease)
            .await
            .unwrap()
            .slot_held
    );
    assert!(matches!(
        service
            .submit(&center.lease, request(second.server_id))
            .await,
        Err(SubmitError::Busy)
    ));
    let restarted = JobService::new(center.store.clone());
    assert!(
        restarted
            .reconcile(id, &center.lease)
            .await
            .unwrap()
            .slot_held
    );
    assert_eq!(a.puts(), 1);
    a.hide_lookup(false);
    a.finish(id, JobPhase::Completed);
    let complete = restarted.reconcile(id, &center.lease).await.unwrap();
    assert!(!complete.slot_held);
    assert_eq!(complete.snapshot.unwrap().phase, JobPhase::Completed);
    assert_eq!(a.puts(), 1);
    let mut unknown = request(second.server_id);
    unknown.job_id = JobId::new();
    b.lose_response(true);
    b.hide_lookup(true);
    b.cancel_first(true);
    restarted
        .submit(&center.lease, unknown.clone())
        .await
        .unwrap();
    let cancelled = restarted
        .cancel(unknown.job_id, &center.lease)
        .await
        .unwrap();
    assert!(!cancelled.slot_held);
    assert_eq!(b.puts(), 1);
    // Duplicate browser submission after terminal acknowledgement never sends another PUT.
    restarted.submit(&center.lease, unknown).await.unwrap();
    assert_eq!(b.puts(), 1);
}
#[tokio::test]
async fn clean_requires_successful_exact_preview_and_observed_service_restoration() {
    let center = fixtures::center().await;
    let (agent, control) = fixtures::job_agent::start(&center).await;
    let target = fixtures::jobs::configured(
        &center,
        agent.credentials.agent_id,
        agent.credentials.endpoint.as_str(),
    )
    .await;
    let service = JobService::new(center.store.clone());
    let preview = request(target.server_id);
    let preview_id = preview.job_id;
    service
        .submit(&center.lease, preview.clone())
        .await
        .unwrap();
    control.finish(preview_id, JobPhase::Completed);
    service.reconcile(preview_id, &center.lease).await.unwrap();
    let mut clean = preview;
    clean.job_id = JobId::new();
    clean.operation = Operation::Clean;
    clean.preview_job_id = Some(preview_id);
    clean.confirmed = true;
    let mut changed = clean.clone();
    changed.scan = Scan::Full;
    assert!(matches!(
        service.submit(&center.lease, changed).await,
        Err(SubmitError::Invalid)
    ));
    service.submit(&center.lease, clean.clone()).await.unwrap();
    control.finish(clean.job_id, JobPhase::Completed);
    control.mail(ObservedMailState::Stopped);
    assert!(
        service
            .reconcile(clean.job_id, &center.lease)
            .await
            .unwrap()
            .slot_held
    );
    control.mail(ObservedMailState::Running);
    assert!(
        !service
            .reconcile(clean.job_id, &center.lease)
            .await
            .unwrap()
            .slot_held
    );
    let history = service
        .history(target.server_id, None, 50, &center.lease)
        .await
        .unwrap();
    assert_eq!(history.jobs.len(), 2);
    assert_eq!(
        history.jobs[0]
            .snapshot
            .as_ref()
            .unwrap()
            .summary
            .remaining_old_files,
        None
    );
    assert!(service
        .history(target.server_id, None, 101, &center.lease)
        .await
        .is_err());
    assert_eq!(control.puts(), 2);
}

#[tokio::test]
async fn logout_after_accept_does_not_cancel_execution_and_next_login_only_reconciles() {
    let center = fixtures::center().await;
    let (agent, control) = fixtures::job_agent::start(&center).await;
    let target = fixtures::jobs::configured(
        &center,
        agent.credentials.agent_id,
        agent.credentials.endpoint.as_str(),
    )
    .await;
    let service = JobService::new(center.store.clone());
    let input = request(target.server_id);
    let id = input.job_id;
    control.hold_response();
    let submitter = service.clone();
    let lease = center.lease.clone();
    let task = tokio::spawn(async move { submitter.submit(&lease, input).await });
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while control.receipt(id).is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    center.authority.revoke(center.lease.generation());
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(result.snapshot.is_none());
    assert!(result.slot_held);
    control.release_response();
    control.finish(id, JobPhase::Interrupted);
    let account = center.store.account().await.unwrap().unwrap();
    let dek = hmail_cleaner_center::vault::open_password(
        &account.material,
        &zeroize::Zeroizing::new("fixture-center-password".into()),
    )
    .unwrap();
    let grant = center.authority.admit(account.revision, dek).unwrap();
    let fresh = center.authority.authenticate(&grant.token).unwrap();
    let restarted = JobService::new(center.store.clone());
    let recovered = restarted.reconcile(id, &fresh).await.unwrap();
    assert_ne!(result.stream_epoch, recovered.stream_epoch);
    assert_eq!(recovered.snapshot.unwrap().phase, JobPhase::Interrupted);
    assert!(!recovered.slot_held);
    assert_eq!(control.puts(), 1);
    assert!(service.get(id, &center.lease).await.is_err());
}
