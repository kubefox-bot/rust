use super::AppState;
use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
const MAX_COOKIE_BYTES: usize = 4096;
pub(super) async fn enforce(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .or_else(|| request.uri().authority().map(|a| a.as_str()));
    if host != Some(state.origin.authority()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if headers
        .get(header::COOKIE)
        .is_some_and(|value| value.len() > MAX_COOKIE_BYTES)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        if origin.as_bytes() != state.origin.as_str().as_bytes() {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        if headers.get("x-hmail-request").map(|h| h.as_bytes()) != Some(b"1".as_slice()) {
            return StatusCode::FORBIDDEN.into_response();
        }
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(';').next());
        if content_type != Some("application/json") {
            return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
        }
    }
    next.run(request).await
}
pub(super) async fn deadline(request: Request, next: Next) -> Response {
    const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            let mut response = StatusCode::REQUEST_TIMEOUT.into_response();
            response.headers_mut().insert(
                header::CONNECTION,
                axum::http::HeaderValue::from_static("close"),
            );
            response
        }
    }
}
pub(super) async fn no_store(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
