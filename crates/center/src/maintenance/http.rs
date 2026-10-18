use crate::app::AppState;
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::Response,
};
pub(crate) async fn admission(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path();
    let path = path.strip_prefix("/api/v1").unwrap_or(path);
    let is_read = matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    let control = matches!(path, "/auth/login" | "/auth/logout" | "/auth/activity")
        || path.starts_with("/jobs/")
            && (path.ends_with("/cancel") || path.ends_with("/reconcile"))
        || path.starts_with("/enrollment/") && path.ends_with("/reconcile");
    let _permit = if is_read {
        // Reads may refresh identities. These are independently tracked at their write boundary.
        None
    } else if control {
        Some(
            state
                .store
                .maintenance()
                .continuation()
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        )
    } else {
        Some(
            state
                .store
                .maintenance()
                .admit()
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        )
    };
    Ok(next.run(request).await)
}
