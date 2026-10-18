use super::{persistence::write_policy, read_policy};
use crate::{auth::http::lease, http::AppState};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use hmail_cleaner_protocol::browser::updates::UpdatePolicy;
use tower_cookies::Cookies;

pub fn routes() -> Router<AppState> {
    Router::new().route("/settings/updates", get(read).patch(update))
}
async fn read(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<UpdatePolicy>, StatusCode> {
    lease(&state, &cookies)?;
    Ok(Json(
        read_policy(state.store.directory()).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
    ))
}
async fn update(
    State(state): State<AppState>,
    cookies: Cookies,
    payload: Result<Json<UpdatePolicy>, JsonRejection>,
) -> Result<Json<UpdatePolicy>, StatusCode> {
    let current = lease(&state, &cookies)?;
    let Json(input) = payload.map_err(|_| StatusCode::BAD_REQUEST)?;
    input.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let _permit = state
        .store
        .maintenance()
        .admit()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let _barrier = state.authority.credential_barrier.lock().await;
    // Reauthenticate after waiting: an expired or replaced generation may not write.
    let live = lease(&state, &cookies)?;
    if live.generation() != current.generation() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    write_policy(state.store.directory(), input).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let idle = state
        .authority
        .idle_minutes()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    state
        .authority
        .set_idle_with_activity(current.generation(), idle)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok(Json(input))
}
