use crate::{
    auth::http::lease,
    http::{errors::ApiError, AppState},
    identity::IdentityService,
    profiles::ProfileService,
};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use hmail_cleaner_protocol::{
    browser::agents::{AgentView, HealthView},
    AgentId,
};
use sqlx::Row;
use tower_cookies::Cookies;
const EXPIRY_WARNING_DAYS: i64 = 14;
pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/agents", get(list))
        .route("/agents/{id}/revoke", post(revoke))
        .route("/agents/{id}/renew", post(renew))
}
async fn list(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<Vec<AgentView>>, ApiError> {
    let current = lease(&state, &cookies)?;
    let servers = ProfileService::new(state.store.clone())
        .servers(&current)
        .await?;
    state.monitor.refresh(&servers, &current);
    let mut views = Vec::with_capacity(servers.len());
    let now = chrono::Utc::now();
    for server in servers {
        let row = sqlx::query("SELECT revoked,expires_at FROM trust WHERE agent_id=?")
            .bind(server.agent_id.to_string())
            .fetch_optional(&state.store.pool)
            .await
            .map_err(|_| ApiError::unavailable())?
            .ok_or_else(ApiError::unavailable)?;
        let revoked: bool = row
            .try_get("revoked")
            .map_err(|_| ApiError::unavailable())?;
        let expires: i64 = row
            .try_get("expires_at")
            .map_err(|_| ApiError::unavailable())?;
        let expires =
            chrono::DateTime::from_timestamp(expires, 0).ok_or_else(ApiError::unavailable)?;
        let observed = state.monitor.view(server.agent_id)?;
        let expired = expires <= now;
        let trusted = !revoked && !expired;
        let health = HealthView {
            online: observed.online && trusted,
            stale: observed.stale || !trusted,
            healthy: observed.healthy.filter(|_| trusted),
            ..observed
        };
        views.push(AgentView {
            agent_id: server.agent_id,
            server_id: server.id,
            name: server.name,
            revoked,
            health,
            certificate_expires_at: Some(expires),
            certificate_expiring: expires <= now + chrono::Duration::days(EXPIRY_WARNING_DAYS),
            certificate_expired: expired,
        });
    }
    Ok(Json(views))
}
async fn revoke(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(agent): Path<AgentId>,
    payload: Result<Json<serde_json::Value>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let current = lease(&state, &cookies)?;
    let Json(_) = payload.map_err(|_| ApiError::bad_request())?;
    IdentityService::new(state.store)
        .revoke_agent(agent, &current)
        .await
        .map_err(|_| ApiError::unavailable())?;
    Ok(StatusCode::NO_CONTENT)
}
async fn renew(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(agent): Path<AgentId>,
    payload: Result<Json<serde_json::Value>, JsonRejection>,
) -> Result<Json<hmail_cleaner_protocol::IdentityReceipt>, ApiError> {
    let current = lease(&state, &cookies)?;
    let Json(_) = payload.map_err(|_| ApiError::bad_request())?;
    let profiles = ProfileService::new(state.store.clone());
    let server = profiles
        .servers(&current)
        .await?
        .into_iter()
        .find(|server| server.agent_id == agent)
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;
    let credentials = profiles.credentials_for_server(server.id, &current).await?;
    if credentials.agent_id != agent {
        return Err(ApiError::unavailable());
    }
    let issued = IdentityService::new(state.store)
        .renew_agent(credentials, &current)
        .await
        .map_err(|error| match error {
            crate::identity::RenewError::Agent(error) => ApiError::from(error),
            crate::identity::RenewError::Identity(_) => ApiError::unavailable(),
        })?;
    Ok(Json(hmail_cleaner_protocol::IdentityReceipt {
        agent_id: agent,
        serial: issued.serial,
    }))
}
