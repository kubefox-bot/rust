use axum::{body::Body, http::Request, Router};
use hmail_cleaner_center::{
    account::{initialize, InitInput, SecretPrompt},
    auth::{Clock, SessionAuthority},
    config::BrowserOrigin,
    http::{router, AppState},
    storage::{CenterStore, DataDirectory, DataDirectoryLease},
    vault::{Dek, SecretText},
};
use sqlx::{sqlite::SqliteConnectOptions, Connection};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tower::ServiceExt;
struct ManualClock(AtomicU64);
impl Clock for ManualClock {
    fn now(&self) -> Duration {
        Duration::from_secs(self.0.load(Ordering::SeqCst))
    }
}
struct Prompt;
impl SecretPrompt for Prompt {
    fn password(&mut self) -> Result<SecretText, String> {
        Ok(SecretText::new("fixture-settings-password".into()))
    }
    fn confirm_recovery(&mut self, _: &SecretText) -> Result<bool, String> {
        Ok(true)
    }
}
struct Fixture {
    root: tempfile::TempDir,
    _directory: DataDirectoryLease,
    store: CenterStore,
    clock: Arc<ManualClock>,
    authority: Arc<SessionAuthority>,
    token: SecretText,
    app: Router,
}
async fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let directory = DataDirectory::acquire(root.path()).unwrap();
    let store = CenterStore::open(&directory).await.unwrap();
    initialize(
        &store,
        &InitInput {
            login: "admin".into(),
        },
        &mut Prompt,
    )
    .await
    .unwrap();
    let clock = Arc::new(ManualClock(AtomicU64::new(0)));
    let authority = Arc::new(SessionAuthority::new(1, 60, clock.clone()).unwrap());
    let grant = authority.admit(1, Dek::new([1; 32])).unwrap();
    let state = AppState::new(
        store.clone(),
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap()
    .with_authority(authority.clone());
    Fixture {
        root,
        _directory: directory,
        store,
        clock,
        authority,
        token: grant.token,
        app: router(state),
    }
}
fn patch(token: &str, minutes: u16) -> Request<Body> {
    Request::patch("/api/v1/settings/session")
        .header("host", "localhost:8443")
        .header("content-type", "application/json")
        .header("x-hmail-request", "1")
        .header("cookie", format!("__Host-hmail-session={token}"))
        .body(Body::from(
            serde_json::json!({"idle_minutes":minutes}).to_string(),
        ))
        .unwrap()
}
#[tokio::test]
async fn saving_shorter_idle_timeout_counts_as_activity_without_activity_request() {
    let f = fixture().await;
    let lease = f.authority.authenticate(&f.token).unwrap();
    f.clock.0.store(1000, Ordering::SeqCst);
    let response = f.app.oneshot(patch(&f.token, 15)).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert!(
        lease.is_live(),
        "successful settings save must count as activity"
    );
    assert!(f.authority.authenticate(&f.token).is_ok());
    assert_eq!(f.authority.idle_minutes().unwrap(), 15);
    f.clock.0.store(1899, Ordering::SeqCst);
    assert!(f.authority.authenticate(&f.token).is_ok());
    f.clock.0.store(1900, Ordering::SeqCst);
    assert!(f.authority.authenticate(&f.token).is_err());
    let restarted = AppState::new(
        f.store,
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(restarted.authority().idle_minutes().unwrap(), 15);
}
#[tokio::test]
async fn failed_settings_persistence_does_not_change_timeout_or_count_as_activity() {
    let f = fixture().await;
    let mut database = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(f.root.path().join("center.sqlite")),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER reject_settings BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'fixture failure'); END").execute(&mut database).await.unwrap();
    f.clock.0.store(1000, Ordering::SeqCst);
    let response = f.app.oneshot(patch(&f.token, 30)).await.unwrap();
    assert_eq!(response.status().as_u16(), 503);
    assert_eq!(f.authority.idle_minutes().unwrap(), 60);
    let persisted: i64 = sqlx::query_scalar("SELECT idle_minutes FROM settings WHERE singleton=1")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(persisted, 15);
    f.clock.0.store(3600, Ordering::SeqCst);
    assert!(f.authority.authenticate(&f.token).is_err());
}
#[tokio::test]
async fn stale_settings_request_cannot_touch_or_change_the_replacement_session() {
    let f = fixture().await;
    f.clock.0.store(1000, Ordering::SeqCst);
    let current = f.authority.admit(1, Dek::new([2; 32])).unwrap();
    f.clock.0.store(1010, Ordering::SeqCst);
    let response = f.app.oneshot(patch(&f.token, 15)).await.unwrap();
    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(f.authority.idle_minutes().unwrap(), 60);
    f.clock.0.store(4600, Ordering::SeqCst);
    assert!(f.authority.authenticate(&current.token).is_err());
}
