use hmail_cleaner_center::agents::HealthTracker;
use hmail_cleaner_protocol::{AgentHealth, AgentId};
use std::time::Duration;
#[test]
fn failed_or_stale_observations_never_claim_fresh_health() {
    let tracker = HealthTracker::new();
    let id = AgentId::new();
    assert!(tracker.view(id, Duration::ZERO).unwrap().stale);
    tracker
        .observed(
            AgentHealth {
                agent_id: id,
                version: "fixture-build".into(),
                protocol_min: 1,
                protocol_max: 1,
                stream_epoch: uuid::Uuid::new_v4(),
                revision: 1.into(),
                healthy: true,
            },
            chrono::Utc::now(),
            Duration::ZERO,
        )
        .unwrap();
    let fresh = tracker.view(id, Duration::from_secs(10)).unwrap();
    assert!(!fresh.stale);
    assert_eq!(fresh.healthy, Some(true));
    let old = tracker.view(id, Duration::from_secs(31)).unwrap();
    assert!(old.stale);
    assert_eq!(old.healthy, None);
    tracker.unreachable(id).unwrap();
    let failed = tracker.view(id, Duration::from_secs(12)).unwrap();
    assert!(!failed.online);
    assert!(failed.stale);
    assert_eq!(failed.healthy, None);
    assert!(failed.observed_at.is_some());
}
