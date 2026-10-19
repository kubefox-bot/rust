use super::{cache, HTML_CACHE};
use crate::http::header_profile::HeaderProfile;
use axum::{
    extract::Request,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use std::{io::ErrorKind, path::Path};
use tokio::io::AsyncReadExt;

const MAX_HTML_BYTES: u64 = 2 * 1024 * 1024;
const GUEST_MARKER: &str = "data-header-profile=\"guest\"";
const ADMIN_MARKER: &str = "data-header-profile=\"admin\"";

pub(super) async fn serve(path: &Path, request: Request) -> Response {
    let result = read(path).await;
    let response = match result {
        Ok(html) => {
            let authenticated = request
                .extensions()
                .get::<HeaderProfile>()
                .is_some_and(|profile| profile.0);
            // Only fixed display markers are substituted; no claims or credentials enter HTML.
            let html = if authenticated {
                html.replacen(GUEST_MARKER, ADMIN_MARKER, 1)
            } else {
                html
            };
            Html(html).into_response()
        }
        Err(status) => status.into_response(),
    };
    cache(response, HTML_CACHE)
}
async fn read(path: &Path) -> Result<String, StatusCode> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::SERVICE_UNAVAILABLE,
        })?;
    let mut bytes = Vec::new();
    file.take(MAX_HTML_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    if bytes.len() as u64 > MAX_HTML_BYTES {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    String::from_utf8(bytes).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
