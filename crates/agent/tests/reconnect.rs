mod fixtures;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use fixtures::{job, Controlled};
use hmail_cleaner_agent::{execution::Worker, http::router, jobs::JobRepository};
use hmail_cleaner_protocol::{AgentId, JobId, JobPhase, JobSnapshot};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tower::ServiceExt;
const TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test]
async fn lost_put_response_and_reconnect_reuse_receipt_without_reexecution() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let executor = Controlled::new(false);
    let agent = AgentId::new();
    let worker = Worker::new(agent, repo, executor.clone());
    let app = router(worker.clone());
    let command = job(agent);
    let id = command.command().job_id;
    let request = || {
        Request::put(format!("/agent/v1/jobs/{id}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(command.command()).unwrap()))
            .unwrap()
    };
    drop(app.clone().oneshot(request()).await.unwrap());
    executor.started.acquire().await.unwrap().forget();
    let repeated = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(repeated.status(), StatusCode::OK);
    let bytes = to_bytes(repeated.into_body(), 128 * 1024).await.unwrap();
    let public = std::str::from_utf8(&bytes).unwrap();
    assert!(!public.contains("fixture-mail-secret"));
    assert!(!public.contains("mail_profile"));
    assert!(!public.contains("Data"));
    executor.finish.add_permits(1);
    tokio::time::timeout(TIMEOUT, async {
        loop {
            let response = app
                .clone()
                .oneshot(
                    Request::get(format!("/agent/v1/jobs/{id}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
            let snapshot: JobSnapshot = serde_json::from_slice(&bytes).unwrap();
            if snapshot.phase == JobPhase::Completed {
                assert!(snapshot.summary.remaining_old_files.is_none());
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(executor.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancel_before_put_and_path_mismatch_never_start_work() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let executor = Controlled::new(false);
    let agent = AgentId::new();
    let app = router(Worker::new(agent, repo, executor.clone()));
    let command = job(agent);
    let id = command.command().job_id;
    let cancelled = app
        .clone()
        .oneshot(
            Request::put(format!("/agent/v1/jobs/{id}/cancel"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::OK);
    let put = |id| {
        Request::put(format!("/agent/v1/jobs/{id}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(command.command()).unwrap()))
            .unwrap()
    };
    let mismatch = app.clone().oneshot(put(JobId::new())).await.unwrap();
    assert_eq!(mismatch.status(), StatusCode::BAD_REQUEST);
    let delayed = app.oneshot(put(id)).await.unwrap();
    let bytes = to_bytes(delayed.into_body(), 128 * 1024).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<JobSnapshot>(&bytes).unwrap().phase,
        JobPhase::Cancelled
    );
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn slow_observer_is_bounded_and_can_reload_the_authoritative_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let worker = Worker::new(AgentId::new(), repo, Controlled::new(false));
    let mut slow = worker.subscribe();
    let first_id = JobId::new();
    worker.cancel(first_id).await.unwrap();
    for _ in 0..64 {
        worker.cancel(JobId::new()).await.unwrap();
    }
    assert!(matches!(
        slow.recv().await,
        Err(tokio::sync::broadcast::error::RecvError::Lagged(_))
    ));
    assert_eq!(
        worker.lookup(first_id).await.unwrap().unwrap().phase,
        JobPhase::Cancelled
    );
}

#[tokio::test]
async fn unfinished_request_body_cannot_occupy_a_connection_without_a_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let app = router(Worker::new(AgentId::new(), repo, Controlled::new(false)));
    tokio::time::pause();
    let body = Body::from_stream(futures_util::stream::pending::<
        Result<axum::body::Bytes, std::io::Error>,
    >());
    let request = Request::post("/agent/v1/resolve")
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(20), app.oneshot(request))
        .await
        .expect("control deadline must finish the response")
        .unwrap();
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}
