#![cfg(unix)]
use astra_updater::policy::due;
use std::time::Duration;
#[test]
fn first_check_is_immediate_and_new_policy_controls_next_check() {
    assert!(due(None, 10, Duration::from_secs(60)));
    assert!(!due(Some(100), 159, Duration::from_secs(60)));
    assert!(due(Some(100), 160, Duration::from_secs(60)));
    assert!(!due(Some(100), 160, Duration::from_secs(300)));
    assert!(due(Some(100), 160, Duration::from_secs(30)));
}
#[test]
fn clock_rollback_does_not_freeze_discovery() {
    assert!(due(Some(100), 90, Duration::from_secs(60)));
}
