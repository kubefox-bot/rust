use super::errors::ApiError;
use axum::{Extension, Json};
use std::path::PathBuf;
#[derive(Clone)]
pub(crate) struct InstallationReader {
    pub directory: PathBuf,
    pub executable: PathBuf,
}
pub(super) async fn get(
    reader: Option<Extension<InstallationReader>>,
) -> Result<Json<hmail_cleaner_protocol::InstallationStatus>, ApiError> {
    let Some(Extension(reader)) = reader else {
        return Err(ApiError::unavailable());
    };
    let status = tokio::task::spawn_blocking(move || {
        crate::service::installation_status(&reader.directory, &reader.executable)
    })
    .await
    .map_err(|_| ApiError::unavailable())?
    .map_err(|_| ApiError::unavailable())?;
    status.map(Json).ok_or_else(ApiError::not_found)
}
