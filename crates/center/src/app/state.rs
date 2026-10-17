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
    pub authority: Arc<SessionAuthority>,
    pub origin: BrowserOrigin,
    pub(crate) limiter: Arc<LoginLimiter>,
    pub(crate) monitor: crate::agents::HealthMonitor,
}
impl AppState {
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
        Ok(Self {
            store,
            authority,
            origin,
            limiter: Arc::new(LoginLimiter::new(clock)),
            monitor,
        })
    }
}
