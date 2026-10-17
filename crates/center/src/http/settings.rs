use super::AppState;
use crate::auth::http::lease;
use axum::{extract::State, http::StatusCode, Json};
use hmail_cleaner_protocol::browser::settings::SessionSettings;
use tower_cookies::Cookies;
pub(super) async fn read(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<SessionSettings>, StatusCode> {
    lease(&state, &cookies)?;
    Ok(Json(SessionSettings {
        idle_minutes: state
            .authority
            .idle_minutes()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
    }))
}
pub(super) async fn update(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(input): Json<SessionSettings>,
) -> Result<Json<SessionSettings>, StatusCode> {
    if !matches!(input.idle_minutes, 15 | 30 | 60 | 1440) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let current = lease(&state, &cookies)?;
    let _barrier = state.authority.credential_barrier.lock().await;
    if !current.is_live() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    sqlx::query("UPDATE settings SET idle_minutes=? WHERE singleton=1")
        .bind(i64::from(input.idle_minutes))
        .execute(&state.store.pool)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    state
        .authority
        .set_idle(current.generation, input.idle_minutes)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok(Json(input))
}
