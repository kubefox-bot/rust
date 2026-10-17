use super::errors::ApiError;
use crate::execution::{AgentError, Executor, Worker};
use axum::{
    extract::{rejection::JsonRejection, State},
    Json,
};
use hmail_cleaner_adapters::{
    filesystem::LocalFiles,
    mysql::{DatabaseConfig, MySqlRepository},
    offline::{verify_data_folder, WindowsMailControl},
};
use hmail_cleaner_core::maintenance::mail_service::MailControl;
use hmail_cleaner_protocol::{ProfileValidationRequest, ProfileValidationResponse};
use std::path::Path;

pub(super) async fn post<E: Executor>(
    State(worker): State<Worker<E>>,
    payload: Result<Json<ProfileValidationRequest>, JsonRejection>,
) -> Result<Json<ProfileValidationResponse>, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::bad_request())?;
    let validated = request.validate().map_err(|_| ApiError::bad_request())?;
    if validated.agent_id() != worker.agent_id() {
        return Err(ApiError::bad_request());
    }
    let profile = validated.profile();
    verify_data_folder(Path::new(&profile.data_dir), Path::new(&profile.hmail_ini))
        .map_err(|_| ApiError::bad_request())?;
    let _files =
        LocalFiles::new(profile.data_dir.clone().into()).map_err(|_| ApiError::bad_request())?;
    let control =
        WindowsMailControl::new(&profile.service_name).map_err(|_| AgentError::Unavailable)?;
    let mail_state = control
        .state()
        .await
        .map_err(|_| AgentError::Unavailable)?
        .into();
    let database =
        DatabaseConfig::from_parts(&profile.mysql).map_err(|_| ApiError::bad_request())?;
    // Schema and connection checks only; no scan, Stop, DELETE or DELETE privilege probe.
    MySqlRepository::connect(&database)
        .await
        .map_err(|_| AgentError::Unavailable)?
        .close()
        .await
        .map_err(|_| AgentError::Unavailable)?;
    Ok(Json(ProfileValidationResponse { mail_state }))
}
