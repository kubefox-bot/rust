use super::Coordinator;
use crate::Error;
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
const MAX_REQUEST_BYTES: usize = 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    client_params: WireIdentity,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireIdentity {
    id: String,
    group: String,
}

#[derive(Clone, Copy)]
enum Operation {
    PreReboot,
    SteadyState,
}
pub fn router(coordinator: Arc<Coordinator>) -> Router {
    Router::new()
        .route("/v1/pre-reboot", post(pre_reboot))
        .route("/v1/steady-state", post(steady_state))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .layer(middleware::from_fn(deadline))
        .with_state(coordinator)
}
async fn deadline(request: Request, next: Next) -> Response {
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(request)).await {
        Ok(response) => response,
        Err(_) => failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Coordinator deadline exceeded",
        ),
    }
}
async fn pre_reboot(
    State(coordinator): State<Arc<Coordinator>>,
    headers: HeaderMap,
    input: Result<Json<Input>, JsonRejection>,
) -> Response {
    dispatch(coordinator, headers, input, Operation::PreReboot).await
}
async fn steady_state(
    State(coordinator): State<Arc<Coordinator>>,
    headers: HeaderMap,
    input: Result<Json<Input>, JsonRejection>,
) -> Response {
    dispatch(coordinator, headers, input, Operation::SteadyState).await
}
async fn dispatch(
    coordinator: Arc<Coordinator>,
    headers: HeaderMap,
    input: Result<Json<Input>, JsonRejection>,
    operation: Operation,
) -> Response {
    if headers
        .get("fleet-lock-protocol")
        .is_none_or(|value| value != "true")
    {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid",
            "FleetLock protocol header required",
        );
    }
    let Ok(Json(input)) = input else {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Invalid FleetLock request",
        );
    };
    if !coordinator.authorized(&input.client_params.id, &input.client_params.group) {
        return failure(
            StatusCode::FORBIDDEN,
            "unauthorized",
            "Unknown FleetLock client",
        );
    }
    let result = match operation {
        Operation::PreReboot => coordinator.pre_reboot().await,
        Operation::SteadyState => coordinator.steady_state().await,
    };
    match result {
        Ok(()) => StatusCode::OK.into_response(),
        Err(Error::Conflict) => failure(StatusCode::LOCKED, "deferred", "Reboot is deferred"),
        Err(_) => failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Reboot coordination unavailable",
        ),
    }
}
fn failure(status: StatusCode, kind: &'static str, value: &'static str) -> Response {
    (status, Json(serde_json::json!({"kind":kind,"value":value}))).into_response()
}
