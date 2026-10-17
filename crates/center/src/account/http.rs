use crate::{
    auth::http::lease,
    http::{errors::ApiError, AppState},
    vault::RecoveryKey,
};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use hmail_cleaner_protocol::browser::auth::{
    ChangePasswordInput, ConfirmRecoveryInput, PrepareRecoveryInput, RecoveryCandidate,
};
use tower_cookies::Cookies;
const CREDENTIAL_BODY_BYTES: usize = 32768;
pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/account/password", post(password))
        .route("/account/recovery/prepare", post(prepare))
        .route("/account/recovery/confirm", post(confirm))
        .layer(DefaultBodyLimit::max(CREDENTIAL_BODY_BYTES))
}
async fn password(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(input): Json<ChangePasswordInput>,
) -> Result<StatusCode, ApiError> {
    state
        .account
        .change_password(&lease(&state, &cookies)?, input)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn prepare(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(input): Json<PrepareRecoveryInput>,
) -> Result<Json<RecoveryCandidate>, ApiError> {
    Ok(Json(
        state
            .account
            .prepare_recovery(&lease(&state, &cookies)?, input.current_password)
            .await?,
    ))
}
async fn confirm(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(input): Json<ConfirmRecoveryInput>,
) -> Result<StatusCode, ApiError> {
    let current = lease(&state, &cookies)?;
    let key = RecoveryKey::parse(&input.recovery_key).map_err(|_| ApiError::bad_request())?;
    state
        .account
        .confirm_recovery(&current, input.candidate_id, &key)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
