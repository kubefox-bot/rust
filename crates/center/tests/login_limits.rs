use hmail_cleaner_center::auth::{Clock, LoginLimiter};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> Duration {
        Duration::from_secs(self.0.load(Ordering::SeqCst))
    }
}
#[test]
fn one_bounded_bucket_refills_without_permanent_lockout() {
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let limiter = LoginLimiter::new(clock.clone());
    for _ in 0..6 {
        assert!(limiter.admit());
    }
    assert!(!limiter.admit());
    clock.0.store(10, Ordering::SeqCst);
    assert!(limiter.admit());
    assert!(!limiter.admit());
    clock.0.store(1000, Ordering::SeqCst);
    for _ in 0..6 {
        assert!(limiter.admit());
    }
    assert!(!limiter.admit());
}
