use crate::{
    auth::{LoginLimiter, MonotonicClock, SessionAuthority},
    config::BrowserOrigin,
    storage::CenterStore,
};
use axum::http::StatusCode;
use std::sync::Arc;
#[derive(Clone)]
pub struct AppState {
    pub store: CenterStore,
    pub(crate) authority: Arc<SessionAuthority>,
    pub origin: BrowserOrigin,
    pub(crate) limiter: Arc<LoginLimiter>,
    pub(crate) monitor: crate::agents::HealthMonitor,
    pub jobs: crate::jobs::JobService,
    pub enrollment: crate::enrollment::EnrollmentService,
    pub(crate) account: crate::account::AccountService,
    pub(crate) static_dir: Option<crate::http::static_files::StaticDir>,
}
impl AppState {
    pub fn with_static_dir(mut self, directory: crate::http::static_files::StaticDir) -> Self {
        self.static_dir = Some(directory);
        self
    }
    pub fn authority(&self) -> &Arc<SessionAuthority> {
        &self.authority
    }
    pub fn with_authority(mut self, authority: Arc<SessionAuthority>) -> Self {
        self.account = crate::account::AccountService::new(self.store.clone(), authority.clone());
        self.authority = authority;
        self
    }
    pub async fn new(store: CenterStore, origin: BrowserOrigin) -> Result<Self, StatusCode> {
        let account = store
            .account()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
        let minutes: i64 =
            sqlx::query_scalar("SELECT idle_minutes FROM settings WHERE singleton=1")
                .fetch_one(&store.pool)
                .await
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let clock = Arc::new(MonotonicClock::default());
        let authority = Arc::new(
            SessionAuthority::new(
                account.revision,
                u16::try_from(minutes).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
                clock.clone(),
            )
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        );
        let monitor = crate::agents::HealthMonitor::new(store.clone());
        let jobs = crate::jobs::JobService::new(store.clone());
        let enrollment = crate::enrollment::EnrollmentService::new(
            store.clone(),
            crate::enrollment::PackageCatalog::new(crate::config::AGENT_PACKAGES_DIRECTORY.into()),
            crate::enrollment::NativeSsh,
        );
        Ok(Self {
            static_dir: None,
            account: crate::account::AccountService::new(store.clone(), authority.clone()),
            store,
            authority,
            origin,
            limiter: Arc::new(LoginLimiter::new(clock)),
            monitor,
            jobs,
            enrollment,
        })
    }
}
