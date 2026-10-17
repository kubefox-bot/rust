use super::{events, health, jobs, profile, resolve};
use crate::execution::{Executor, Worker};
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post, put},
    Router,
};
const MAX_JSON_BYTES: usize = 128 * 1024;
/// Serve exclusively through the agent's mTLS acceptor; exposed for in-process tests.
pub fn router<E: Executor>(worker: Worker<E>) -> Router {
    Router::new()
        .route("/agent/v1/health", get(health::get::<E>))
        .route("/agent/v1/installation", get(super::installation::get))
        .route("/agent/v1/identity/csr", post(super::identity::csr))
        .route("/agent/v1/identity", put(super::identity::install::<E>))
        .route("/agent/v1/service/state", post(super::service::post::<E>))
        .route("/agent/v1/resolve", post(resolve::post::<E>))
        .route("/agent/v1/profile/validate", post(profile::post::<E>))
        .route(
            "/agent/v1/jobs/{job_id}",
            put(jobs::put::<E>).get(jobs::get::<E>),
        )
        .route("/agent/v1/jobs/{job_id}/cancel", put(jobs::cancel::<E>))
        .route("/agent/v1/jobs/{job_id}/events", get(events::get::<E>))
        .layer(DefaultBodyLimit::max(MAX_JSON_BYTES))
        .layer(axum::middleware::from_fn(super::policy::control))
        .with_state(worker)
}
