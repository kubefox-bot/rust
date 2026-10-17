use crate::{
    auth::http::lease,
    http::{errors::ApiError, AppState},
};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    routing::{get, post},
    Json, Router,
};
use hmail_cleaner_protocol::{
    browser::jobs::{JobView, SubmitRequest},
    JobId, ServerId,
};
use tower_cookies::Cookies;
pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/servers/{id}/jobs", post(submit))
        .route("/servers/{id}/cleanup-context", get(context))
        .route("/jobs/{id}", get(read))
        .route("/jobs/{id}/cancel", post(cancel))
        .route("/jobs/{id}/reconcile", post(reconcile))
        .route("/events", get(super::event_http::events))
}
async fn context(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(server): Path<ServerId>,
) -> Result<Json<hmail_cleaner_protocol::browser::jobs::CleanupContext>, ApiError> {
    Ok(Json(
        state
            .jobs
            .cleanup_context(server, &lease(&state, &cookies)?)
            .await?,
    ))
}
async fn submit(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(server): Path<ServerId>,
    payload: Result<Json<SubmitRequest>, JsonRejection>,
) -> Result<Json<JobView>, ApiError> {
    let current = lease(&state, &cookies)?;
    let Json(input) = payload.map_err(|_| ApiError::bad_request())?;
    if input.server_id != server {
        return Err(ApiError::bad_request());
    }
    let view = state.jobs.submit(&current, input).await?;
    state.jobs.resume(&current);
    Ok(Json(view))
}
async fn read(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(id): Path<JobId>,
) -> Result<Json<JobView>, ApiError> {
    let current = lease(&state, &cookies)?;
    Ok(Json(state.jobs.get(id, &current).await?))
}
async fn cancel(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(id): Path<JobId>,
) -> Result<Json<JobView>, ApiError> {
    let current = lease(&state, &cookies)?;
    Ok(Json(state.jobs.cancel(id, &current).await?))
}
async fn reconcile(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(id): Path<JobId>,
) -> Result<Json<JobView>, ApiError> {
    let current = lease(&state, &cookies)?;
    Ok(Json(state.jobs.reconcile(id, &current).await?))
}
