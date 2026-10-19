use crate::{
    auth::http::lease,
    http::{errors::ApiError, AppState},
};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use hmail_cleaner_protocol::{
    browser::enrollment::{
        AgentPackageView, EnrollmentView, HostKeyChallenge, InstallRequest, SshTarget,
        UpdateRequest,
    },
    AgentId,
};
use tower_cookies::Cookies;
const MAX_ENROLLMENT_BYTES: usize = 128 * 1024;
pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/enrollment", get(recent))
        .route("/enrollment/packages", get(packages))
        .route("/enrollment/probe", post(probe))
        .route("/enrollment/install", post(install))
        .route("/enrollment/{id}", get(read))
        .route("/enrollment/{id}/reconcile", post(reconcile))
        .route("/agents/{id}/update", post(update))
        .layer(DefaultBodyLimit::max(MAX_ENROLLMENT_BYTES))
}
async fn recent(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<Vec<EnrollmentView>>, ApiError> {
    let current = lease(&state, &cookies)?;
    Ok(Json(state.enrollment.recent(&current).await?))
}
async fn packages(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<Vec<AgentPackageView>>, ApiError> {
    let _current = lease(&state, &cookies)?;
    Ok(Json(state.enrollment.catalog.list().await?))
}
async fn probe(
    State(state): State<AppState>,
    cookies: Cookies,
    payload: Result<Json<SshTarget>, JsonRejection>,
) -> Result<Json<HostKeyChallenge>, ApiError> {
    let current = lease(&state, &cookies)?;
    let Json(input) = payload.map_err(|_| ApiError::bad_request())?;
    let _permit = state
        .enrollment
        .probes
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::busy())?;
    Ok(Json(super::NativeSsh::probe(input, &current).await?))
}
async fn install(
    State(state): State<AppState>,
    cookies: Cookies,
    payload: Result<Json<InstallRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<EnrollmentView>), ApiError> {
    let current = lease(&state, &cookies)?;
    let Json(input) = payload.map_err(|_| ApiError::bad_request())?;
    Ok((
        StatusCode::ACCEPTED,
        Json(state.enrollment.install(&current, input).await?),
    ))
}
async fn update(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(id): Path<AgentId>,
    payload: Result<Json<UpdateRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<EnrollmentView>), ApiError> {
    let current = lease(&state, &cookies)?;
    let Json(input) = payload.map_err(|_| ApiError::bad_request())?;
    Ok((
        StatusCode::ACCEPTED,
        Json(state.enrollment.update(&current, id, input).await?),
    ))
}
async fn read(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<EnrollmentView>, ApiError> {
    let current = lease(&state, &cookies)?;
    Ok(Json(state.enrollment.get(id, &current).await?))
}
async fn reconcile(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<EnrollmentView>, ApiError> {
    let current = lease(&state, &cookies)?;
    Ok(Json(state.enrollment.reconcile(id, &current).await?))
}
