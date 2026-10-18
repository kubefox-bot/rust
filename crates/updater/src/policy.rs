use std::time::Duration;
/// A wall-clock rollback triggers a check instead of indefinitely delaying updates.
pub fn due(last_check: Option<u64>, now: u64, interval: Duration) -> bool {
    match last_check {
        None => true,
        Some(previous) => now < previous || now.saturating_sub(previous) >= interval.as_secs(),
    }
}
