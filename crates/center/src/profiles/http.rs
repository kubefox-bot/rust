use super::{ProfileService, SelectedProfile};
use crate::{
    agents::AgentClient,
    auth::{http::lease, SessionLease},
    http::{errors::ApiError, AppState},
};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Path, State},
    routing::{get, post},
    Json, Router,
};
use hmail_cleaner_protocol::{
    browser::profiles::{EditableProfile, ProfileVersion, SaveProfileRequest, ServerView},
    ProfileValidationResponse, ResolveRequest, ResolvedCutoff, RetentionRequest, ServerId,
    PROTOCOL_VERSION,
};
use tower_cookies::Cookies;
const PROFILE_REQUEST_BYTES: usize = 128 * 1024;
pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/servers", get(servers))
        .route("/servers/{id}/profile", get(read).patch(save))
        .route("/servers/{id}/validate", post(validate))
        .route("/servers/{id}/resolve", post(resolve))
        .layer(DefaultBodyLimit::max(PROFILE_REQUEST_BYTES))
}
async fn servers(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<Vec<ServerView>>, ApiError> {
    let current = lease(&state, &cookies)?;
    Ok(Json(
        ProfileService::new(state.store).servers(&current).await?,
    ))
}
async fn read(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(server): Path<ServerId>,
) -> Result<Json<EditableProfile>, ApiError> {
    let current = lease(&state, &cookies)?;
    Ok(Json(
        ProfileService::new(state.store)
            .profile_for_server(server, &current)
            .await?,
    ))
}
async fn save(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(server): Path<ServerId>,
    payload: Result<Json<SaveProfileRequest>, JsonRejection>,
) -> Result<Json<ProfileVersion>, ApiError> {
    let current = lease(&state, &cookies)?;
    let Json(input) = payload.map_err(|_| ApiError::bad_request())?;
    let service = ProfileService::new(state.store);
    let profile = service.profile_for_server(server, &current).await?;
    Ok(Json(
        service
            .save(
                profile.id,
                input.expected_version.get(),
                input.profile,
                &current,
            )
            .await?,
    ))
}
async fn selected(
    state: &AppState,
    server: ServerId,
    lease: &SessionLease,
) -> Result<SelectedProfile, ApiError> {
    let service = ProfileService::new(state.store.clone());
    let profile = service.profile_for_server(server, lease).await?;
    Ok(service
        .for_job(server, profile.id, profile.version.get(), lease)
        .await?)
}
async fn validate(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(server): Path<ServerId>,
    payload: Result<Json<serde_json::Value>, JsonRejection>,
) -> Result<Json<ProfileValidationResponse>, ApiError> {
    let current = lease(&state, &cookies)?;
    let Json(_) = payload.map_err(|_| ApiError::bad_request())?;
    let profile = selected(&state, server, &current).await?;
    let client = AgentClient::new(&state.store, profile.credentials, &current).await?;
    Ok(Json(client.validate_profile(&profile.mail_profile).await?))
}
async fn resolve(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(server): Path<ServerId>,
    payload: Result<Json<RetentionRequest>, JsonRejection>,
) -> Result<Json<ResolvedCutoff>, ApiError> {
    let current = lease(&state, &cookies)?;
    let Json(retention) = payload.map_err(|_| ApiError::bad_request())?;
    let profile = selected(&state, server, &current).await?;
    let request = ResolveRequest {
        protocol_version: PROTOCOL_VERSION,
        agent_id: profile.credentials.agent_id,
        retention,
    };
    let client = AgentClient::new(&state.store, profile.credentials, &current).await?;
    Ok(Json(client.resolve(&request).await?))
}
