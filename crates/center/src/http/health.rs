use super::AppState;
use axum::{extract::State, http::header, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HealthStatus {
    Ok,
    Unavailable,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Health {
    pub status: HealthStatus,
    pub build_version: String,
    pub schema_version: i64,
}
pub(crate) async fn read(
    State(state): State<AppState>,
) -> (
    axum::http::StatusCode,
    [(header::HeaderName, &'static str); 1],
    Json<Health>,
) {
    let healthy = state
        .journal
        .as_ref()
        .is_none_or(|journal| journal.is_healthy());
    (
        if healthy {
            axum::http::StatusCode::OK
        } else {
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        },
        [(header::CACHE_CONTROL, "no-store")],
        Json(Health {
            status: if healthy {
                HealthStatus::Ok
            } else {
                HealthStatus::Unavailable
            },
            build_version: state.build_version.into(),
            schema_version: crate::storage::SCHEMA_VERSION,
        }),
    )
}
