pub mod fixtures;
use hmail_cleaner_center::jobs::JobService;
use hmail_cleaner_protocol::{
    browser::jobs::SubmitRequest, JobId, JobPhase, Operation, RunLimits, Scan, ServicePolicy,
};
#[tokio::test]
async fn observer_persists_a_terminal_sse_without_waiting_for_the_retry_interval() {
    let center = fixtures::center().await;
    let (agent, control) = fixtures::job_agent::start(&center).await;
    let target = fixtures::jobs::configured(
        &center,
        agent.credentials.agent_id,
        agent.credentials.endpoint.as_str(),
    )
    .await;
    let jobs = JobService::new(center.store.clone());
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
    let id = input.job_id;
    jobs.submit(&center.lease, input).await.unwrap();
    jobs.resume(&center.lease);
    jobs.resume(&center.lease);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while control.subscriptions() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(control.subscriptions(), 1);
    control.finish(id, JobPhase::Completed);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let view = jobs.get(id, &center.lease).await.unwrap();
            if !view.slot_held {
                assert_eq!(view.snapshot.unwrap().phase, JobPhase::Completed);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(control.puts(), 1);
    center.authority.revoke(center.lease.generation());
}

#[tokio::test]
async fn an_idle_login_follower_wakes_when_a_job_is_submitted() {
    let center = fixtures::center().await;
    let jobs = JobService::new(center.store.clone());
    // Login starts the follower before there is an agent or a job to observe.
    jobs.resume(&center.lease);
    let (agent, control) = fixtures::job_agent::start(&center).await;
    let target = fixtures::jobs::configured(
        &center,
        agent.credentials.agent_id,
        agent.credentials.endpoint.as_str(),
    )
    .await;
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
    let id = input.job_id;
    jobs.submit(&center.lease, input).await.unwrap();
    jobs.resume(&center.lease);
    jobs.resume(&center.lease);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while control.subscriptions() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("a newly submitted job must wake the existing idle follower");
    assert_eq!(control.subscriptions(), 1);
    control.finish(id, JobPhase::Completed);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let view = jobs.get(id, &center.lease).await.unwrap();
            if !view.slot_held {
                assert_eq!(view.snapshot.unwrap().phase, JobPhase::Completed);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(control.puts(), 1);
    center.authority.revoke(center.lease.generation());
}
