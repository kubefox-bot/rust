pub mod fixtures;
use hmail_cleaner_center::jobs::{EventHub, StreamItem};
use hmail_cleaner_protocol::{
    browser::jobs::{DispatchState, JobView, SubmitRequest},
    JobId, Operation, RunLimits, Scan, ServerId, ServicePolicy,
};
fn view(server: ServerId) -> JobView {
    let id = JobId::new();
    JobView {
        job_id: id,
        intent: SubmitRequest {
            job_id: id,
            server_id: server,
            profile_version: 2.into(),
            operation: Operation::Preview,
            scan: Scan::Fast,
            resolved_cutoff: fixtures::jobs::cutoff(),
            service_policy: ServicePolicy::Manage,
            limits: RunLimits::default(),
            preview_job_id: None,
            confirmed: false,
        },
        dispatch: DispatchState::Unknown,
        snapshot: None,
        slot_held: true,
        stream_epoch: uuid::Uuid::new_v4(),
        revision: 1.into(),
    }
}
#[tokio::test]
async fn slow_subscriber_resyncs_with_bounded_frames_and_connection_slots() {
    let hub = EventHub::default();
    let mut subscriptions = (0..16)
        .map(|_| hub.subscribe().unwrap())
        .collect::<Vec<_>>();
    assert!(hub.subscribe().is_err());
    for _ in 0..257 {
        hub.publish(view(ServerId::new()));
    }
    assert!(matches!(
        subscriptions[0].recv().await,
        Some(StreamItem::Resync)
    ));
    let Some(StreamItem::Snapshot(event)) = subscriptions[0].recv().await else {
        panic!("snapshot expected")
    };
    assert!(serde_json::to_vec(&*event).unwrap().len() <= 4096);
    subscriptions.pop();
    assert!(hub.subscribe().is_ok());
}
#[tokio::test]
async fn http_stream_filters_server_scope_and_closes_on_logout() {
    use axum::{body::Body, http::Request};
    use futures_util::StreamExt;
    use tower::ServiceExt;
    let center = fixtures::center().await;
    let server = ServerId::new();
    let mut state = hmail_cleaner_center::http::AppState::new(
        center.store.clone(),
        hmail_cleaner_center::config::BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap();
    state = state.with_authority(center.authority.clone());
    let hub = state.jobs.events().clone();
    let app = hmail_cleaner_center::http::router(state);
    let response = app
        .oneshot(
            Request::get(format!("/api/v1/events?server_id={server}"))
                .header("host", "localhost:8443")
                .header(
                    "cookie",
                    format!("__Host-hmail-session={}", center.token.as_str()),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let mut body = response.into_body().into_data_stream();
    assert!(
        String::from_utf8(body.next().await.unwrap().unwrap().to_vec())
            .unwrap()
            .contains("resync")
    );
    hub.publish(view(ServerId::new()));
    let expected = view(server);
    let id = expected.job_id;
    hub.publish(expected);
    let bytes = body.next().await.unwrap().unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains(&id.to_string()));
    center.authority.revoke(center.lease.generation());
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), body.next())
            .await
            .unwrap()
            .is_none()
    );
}
