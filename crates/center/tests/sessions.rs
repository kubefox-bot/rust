use hmail_cleaner_center::{
    auth::{Clock, SessionAuthority},
    vault::Dek,
};
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
fn replacements_old_logout_and_idle_timer_are_generation_safe() {
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let authority = SessionAuthority::new(1, 15, clock.clone()).unwrap();
    let first = authority.admit(1, Dek::new([1; 32])).unwrap();
    let lease = authority.authenticate(&first.token).unwrap();
    let second = authority.admit(1, Dek::new([2; 32])).unwrap();
    assert!(authority.authenticate(&first.token).is_err());
    assert!(lease.with_dek(|_| ()).is_err());
    assert!(!authority.revoke(first.generation));
    assert!(authority.authenticate(&second.token).is_ok());
    clock.0.store(901, Ordering::SeqCst);
    authority.expire();
    assert!(authority.authenticate(&second.token).is_err());
}
#[test]
fn stale_credentials_and_malformed_tokens_cannot_replace_a_session() {
    let authority = SessionAuthority::new(2, 15, Arc::new(TestClock(AtomicU64::new(0)))).unwrap();
    let current = authority.admit(2, Dek::new([1; 32])).unwrap();
    assert!(authority.admit(1, Dek::new([2; 32])).is_err());
    assert!(authority.authenticate(&current.token).is_ok());
    assert!(authority.authenticate(&"x".repeat(100000)).is_err());
}
#[test]
fn timeout_change_uses_last_activity_and_all_variants_are_explicit() {
    for minutes in [15, 30, 60, 1440] {
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let authority = SessionAuthority::new(1, minutes, clock.clone()).unwrap();
        let grant = authority.admit(1, Dek::new([1; 32])).unwrap();
        clock.0.store(35, Ordering::SeqCst);
        authority.touch(grant.generation).unwrap();
        clock.0.store(35 + minutes as u64 * 60, Ordering::SeqCst);
        authority.expire();
        assert!(authority.authenticate(&grant.token).is_err());
    }
    assert!(SessionAuthority::new(1, 16, Arc::new(TestClock(AtomicU64::new(0)))).is_err());
}
#[test]
fn delayed_successful_login_cannot_restore_an_older_generation() {
    let authority = SessionAuthority::new(1, 15, Arc::new(TestClock(AtomicU64::new(0)))).unwrap();
    let older = authority.begin_login().unwrap();
    let newer = authority.begin_login().unwrap();
    let grant = authority.admit_ticket(newer, 1, Dek::new([2; 32])).unwrap();
    assert!(authority.admit_ticket(older, 1, Dek::new([1; 32])).is_err());
    assert!(authority.authenticate(&grant.token).is_ok());
}
#[test]
fn reducing_timeout_expires_from_last_activity_and_revokes_existing_leases() {
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let authority = SessionAuthority::new(1, 60, clock.clone()).unwrap();
    let grant = authority.admit(1, Dek::new([1; 32])).unwrap();
    let lease = authority.authenticate(&grant.token).unwrap();
    let revoked = lease.revocation();
    clock.0.store(1000, Ordering::SeqCst);
    authority.set_idle(grant.generation, 15).unwrap();
    assert!(*revoked.borrow());
    assert!(lease.with_dek(|_| ()).is_err());
    assert!(authority.authenticate(&grant.token).is_err());
}
#[test]
fn dropping_authority_revokes_retained_leases() {
    let authority = SessionAuthority::new(1, 15, Arc::new(TestClock(AtomicU64::new(0)))).unwrap();
    let grant = authority.admit(1, Dek::new([1; 32])).unwrap();
    let lease = authority.authenticate(&grant.token).unwrap();
    drop(authority);
    assert!(lease.with_dek(|_| ()).is_err());
}
