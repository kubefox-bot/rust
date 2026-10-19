use crate::vault::{Dek, Entropy, SecretText, SystemEntropy};
use base64ct::{Base64UrlUnpadded, Encoding};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tokio::sync::watch;
const TOKEN_BYTES: usize = 32;
const TOKEN_CHARS: usize = 43;
const ACTIVITY_INTERVAL: Duration = Duration::from_secs(30);
const SECONDS_PER_MINUTE: u64 = 60;
pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;
}
pub struct MonotonicClock(Instant);
impl Default for MonotonicClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl Clock for MonotonicClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}
#[derive(Debug, thiserror::Error)]
#[error("session is unavailable or expired")]
pub struct AuthError;
struct KeyOwner {
    key: Mutex<Option<Dek>>,
    revoked: watch::Sender<bool>,
}
impl KeyOwner {
    fn revoke(&self) {
        self.key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        self.revoked.send_replace(true);
    }
}
impl Drop for KeyOwner {
    fn drop(&mut self) {
        self.revoked.send_replace(true);
    }
}
#[derive(Clone)]
pub struct SessionLease {
    generation: u64,
    account_revision: i64,
    owner: Arc<KeyOwner>,
}
impl SessionLease {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn account_revision(&self) -> i64 {
        self.account_revision
    }
    /// Only synchronous cryptographic operations may run here; no network waits or authority re-entry.
    pub fn with_dek<T>(&self, operation: impl FnOnce(&Dek) -> T) -> Result<T, AuthError> {
        let key = self.owner.key.lock().map_err(|_| AuthError)?;
        Ok(operation(key.as_ref().ok_or(AuthError)?))
    }
    pub fn revocation(&self) -> watch::Receiver<bool> {
        self.owner.revoked.subscribe()
    }
    pub fn is_live(&self) -> bool {
        !*self.owner.revoked.borrow()
    }
}
pub struct SessionGrant {
    pub token: SecretText,
    pub generation: u64,
}
struct Session {
    fingerprint: [u8; 32],
    generation: u64,
    last_activity: Duration,
    lease: SessionLease,
}
struct State {
    closed: bool,
    revision: i64,
    next_ticket: u64,
    admitted_ticket: u64,
    generation: u64,
    idle: Duration,
    current: Option<Session>,
}
pub struct SessionAuthority {
    state: Mutex<State>,
    clock: Arc<dyn Clock>,
    pub(crate) credential_barrier: tokio::sync::Mutex<()>,
}
fn idle(minutes: u16) -> Result<Duration, AuthError> {
    match minutes {
        15 | 30 | 60 | 1440 => Ok(Duration::from_secs(u64::from(minutes) * SECONDS_PER_MINUTE)),
        _ => Err(AuthError),
    }
}
fn fingerprint(token: &str) -> Result<[u8; 32], AuthError> {
    if token.len() != TOKEN_CHARS {
        return Err(AuthError);
    }
    let mut bytes = zeroize::Zeroizing::new([0u8; TOKEN_BYTES]);
    Base64UrlUnpadded::decode(token, &mut *bytes).map_err(|_| AuthError)?;
    let mut result = [0; 32];
    result.copy_from_slice(ring::digest::digest(&ring::digest::SHA256, &*bytes).as_ref());
    Ok(result)
}
fn expire(state: &mut State, now: Duration) {
    if state
        .current
        .as_ref()
        .is_some_and(|session| now.saturating_sub(session.last_activity) >= state.idle)
    {
        if let Some(session) = state.current.take() {
            session.lease.owner.revoke();
        }
    }
}
impl SessionAuthority {
    pub(crate) fn advance_revision(&self, expected: i64, next: i64) -> Result<(), AuthError> {
        let mut state = self.state.lock().map_err(|_| AuthError)?;
        if state.closed || state.revision != expected || expected.checked_add(1) != Some(next) {
            return Err(AuthError);
        }
        if let Some(session) = state.current.take() {
            session.lease.owner.revoke();
        }
        state.revision = next;
        Ok(())
    }
    pub(crate) fn shutdown(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.closed = true;
        if let Some(session) = state.current.take() {
            session.lease.owner.revoke();
        }
    }
    pub fn new(revision: i64, minutes: u16, clock: Arc<dyn Clock>) -> Result<Self, AuthError> {
        if revision <= 0 {
            return Err(AuthError);
        }
        Ok(Self {
            state: Mutex::new(State {
                closed: false,
                revision,
                next_ticket: 0,
                admitted_ticket: 0,
                generation: 0,
                idle: idle(minutes)?,
                current: None,
            }),
            clock,
            credential_barrier: tokio::sync::Mutex::new(()),
        })
    }
    pub fn begin_login(&self) -> Result<u64, AuthError> {
        let mut state = self.state.lock().map_err(|_| AuthError)?;
        if state.closed {
            return Err(AuthError);
        }
        state.next_ticket = state.next_ticket.checked_add(1).ok_or(AuthError)?;
        Ok(state.next_ticket)
    }
    pub fn admit(&self, revision: i64, dek: Dek) -> Result<SessionGrant, AuthError> {
        self.admit_ticket(self.begin_login()?, revision, dek)
    }
    pub fn admit_ticket(
        &self,
        ticket: u64,
        revision: i64,
        dek: Dek,
    ) -> Result<SessionGrant, AuthError> {
        let mut bytes = zeroize::Zeroizing::new([0; TOKEN_BYTES]);
        SystemEntropy.fill(&mut *bytes).map_err(|_| AuthError)?;
        let token = SecretText::new(Base64UrlUnpadded::encode_string(&*bytes));
        let hash = fingerprint(&token)?;
        let mut state = self.state.lock().map_err(|_| AuthError)?;
        if state.closed
            || revision != state.revision
            || ticket <= state.admitted_ticket
            || ticket > state.next_ticket
        {
            return Err(AuthError);
        }
        let generation = state.generation.checked_add(1).ok_or(AuthError)?;
        let (revoked, _) = watch::channel(false);
        let lease = SessionLease {
            generation,
            account_revision: revision,
            owner: Arc::new(KeyOwner {
                key: Mutex::new(Some(dek)),
                revoked,
            }),
        };
        if let Some(old) = state.current.take() {
            old.lease.owner.revoke();
        }
        state.generation = generation;
        state.admitted_ticket = ticket;
        state.current = Some(Session {
            fingerprint: hash,
            generation,
            last_activity: self.clock.now(),
            lease,
        });
        Ok(SessionGrant { token, generation })
    }
    pub fn authenticate(&self, token: &str) -> Result<SessionLease, AuthError> {
        let hash = fingerprint(token)?;
        let mut state = self.state.lock().map_err(|_| AuthError)?;
        expire(&mut state, self.clock.now());
        let session = state.current.as_ref().ok_or(AuthError)?;
        if !bool::from(hash.ct_eq(&session.fingerprint)) {
            return Err(AuthError);
        }
        Ok(session.lease.clone())
    }
    pub fn revoke(&self, generation: u64) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if !state
            .current
            .as_ref()
            .is_some_and(|s| s.generation == generation)
        {
            return false;
        }
        if let Some(session) = state.current.take() {
            session.lease.owner.revoke();
        }
        true
    }
    pub fn expire(&self) {
        if let Ok(mut state) = self.state.lock() {
            expire(&mut state, self.clock.now());
        }
    }
    pub fn touch(&self, generation: u64) -> Result<(), AuthError> {
        let mut state = self.state.lock().map_err(|_| AuthError)?;
        let now = self.clock.now();
        expire(&mut state, now);
        let session = state
            .current
            .as_mut()
            .filter(|s| s.generation == generation)
            .ok_or(AuthError)?;
        if now.saturating_sub(session.last_activity) >= ACTIVITY_INTERVAL {
            session.last_activity = now;
        }
        Ok(())
    }
    pub fn set_idle(&self, generation: u64, minutes: u16) -> Result<(), AuthError> {
        let duration = idle(minutes)?;
        let mut state = self.state.lock().map_err(|_| AuthError)?;
        expire(&mut state, self.clock.now());
        if !state
            .current
            .as_ref()
            .is_some_and(|s| s.generation == generation)
        {
            return Err(AuthError);
        }
        state.idle = duration;
        expire(&mut state, self.clock.now());
        Ok(())
    }
    /// Apply a persisted setting and record its activity as one generation-checked change.
    pub(crate) fn set_idle_with_activity(
        &self,
        generation: u64,
        minutes: u16,
    ) -> Result<(), AuthError> {
        let duration = idle(minutes)?;
        let mut state = self.state.lock().map_err(|_| AuthError)?;
        let now = self.clock.now();
        expire(&mut state, now);
        let session = state
            .current
            .as_mut()
            .filter(|session| session.generation == generation)
            .ok_or(AuthError)?;
        session.last_activity = now;
        state.idle = duration;
        Ok(())
    }
    pub fn idle_minutes(&self) -> Result<u16, AuthError> {
        Ok((self.state.lock().map_err(|_| AuthError)?.idle.as_secs() / SECONDS_PER_MINUTE) as u16)
    }
}
impl Drop for SessionAuthority {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_rejects_inflight_login_completion_and_never_reopens_admission() {
        let authority =
            Arc::new(SessionAuthority::new(1, 15, Arc::new(MonotonicClock::default())).unwrap());
        let grant = authority.admit(1, Dek::new([1; 32])).unwrap();
        let lease = authority.authenticate(&grant.token).unwrap();
        let ticket = authority.begin_login().unwrap();
        let (finish, pending) = tokio::sync::oneshot::channel::<Dek>();
        let waiting = authority.clone();
        let completion =
            tokio::spawn(async move { waiting.admit_ticket(ticket, 1, pending.await.unwrap()) });
        authority.shutdown();
        finish.send(Dek::new([2; 32])).unwrap();
        assert!(completion.await.unwrap().is_err());
        assert!(!lease.is_live());
        assert!(lease.with_dek(|_| ()).is_err());
        assert!(authority.authenticate(&grant.token).is_err());
        assert!(!authority.revoke(grant.generation));
        assert!(authority.begin_login().is_err());
        let _ = authority.advance_revision(1, 2);
        assert!(authority.begin_login().is_err());
        assert!(authority.admit(2, Dek::new([3; 32])).is_err());
    }
}

#[cfg(test)]
mod settings_activity_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestClock(AtomicU64);
    impl Clock for TestClock {
        fn now(&self) -> Duration {
            Duration::from_secs(self.0.load(Ordering::SeqCst))
        }
    }

    #[test]
    fn settings_activity_is_not_coalesced_with_a_recent_touch() {
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let authority = SessionAuthority::new(1, 60, clock.clone()).unwrap();
        let grant = authority.admit(1, Dek::new([1; 32])).unwrap();
        clock.0.store(1000, Ordering::SeqCst);
        authority.touch(grant.generation).unwrap();
        clock.0.store(1010, Ordering::SeqCst);
        authority
            .set_idle_with_activity(grant.generation, 15)
            .unwrap();
        clock.0.store(1909, Ordering::SeqCst);
        assert!(authority.authenticate(&grant.token).is_ok());
        clock.0.store(1910, Ordering::SeqCst);
        assert!(authority.authenticate(&grant.token).is_err());
    }

    #[test]
    fn stale_settings_completion_cannot_change_or_touch_a_new_generation() {
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let authority = SessionAuthority::new(1, 60, clock.clone()).unwrap();
        let old = authority.admit(1, Dek::new([1; 32])).unwrap();
        clock.0.store(1000, Ordering::SeqCst);
        let current = authority.admit(1, Dek::new([2; 32])).unwrap();
        clock.0.store(1010, Ordering::SeqCst);
        assert!(authority
            .set_idle_with_activity(old.generation, 15)
            .is_err());
        assert_eq!(authority.idle_minutes().unwrap(), 60);
        clock.0.store(4599, Ordering::SeqCst);
        assert!(authority.authenticate(&current.token).is_ok());
        clock.0.store(4600, Ordering::SeqCst);
        assert!(authority.authenticate(&current.token).is_err());
    }

    #[test]
    fn expired_settings_completion_never_revives_a_session() {
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let authority = SessionAuthority::new(1, 60, clock.clone()).unwrap();
        let grant = authority.admit(1, Dek::new([1; 32])).unwrap();
        let lease = authority.authenticate(&grant.token).unwrap();
        clock.0.store(3600, Ordering::SeqCst);
        assert!(authority
            .set_idle_with_activity(grant.generation, 1440)
            .is_err());
        assert!(!lease.is_live());
        assert!(authority.authenticate(&grant.token).is_err());
        assert_eq!(authority.idle_minutes().unwrap(), 60);
    }
}
