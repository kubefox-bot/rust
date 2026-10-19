use axum::{
    extract::Request,
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use hmail_cleaner_protocol::{ErrorCode, PublicError};
use std::time::Duration;
const CONTROL_DEADLINE: Duration = Duration::from_secs(15);

pub(super) async fn control(request: Request, next: Next) -> Response {
    // Covers admission and body reads; a returned SSE stream and the worker have
    // independent lifetimes and are not limited by this request deadline.
    let mut response = match tokio::time::timeout(CONTROL_DEADLINE, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            let mut response = (
                StatusCode::REQUEST_TIMEOUT,
                Json(PublicError::new(ErrorCode::Unavailable)),
            )
                .into_response();
            response
                .headers_mut()
                .insert(header::CONNECTION, HeaderValue::from_static("close"));
            response
        }
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
