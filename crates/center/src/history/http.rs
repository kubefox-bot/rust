use crate::{
    auth::http::lease,
    http::{errors::ApiError, AppState},
};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use hmail_cleaner_protocol::{browser::jobs::HistoryPage, Counter, ServerId};
use serde::Deserialize;
use tower_cookies::Cookies;
#[derive(Deserialize)]
struct Page {
    cursor: Option<Counter>,
    limit: Option<u16>,
}
pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/servers/{id}/history", get(read))
}
async fn read(
    State(state): State<AppState>,
    cookies: Cookies,
    Path(id): Path<ServerId>,
    Query(page): Query<Page>,
) -> Result<Json<HistoryPage>, ApiError> {
    let current = lease(&state, &cookies)?;
    Ok(Json(
        state
            .jobs
            .history(
                id,
                page.cursor,
                page.limit.unwrap_or(super::query::DEFAULT_PAGE_SIZE),
                &current,
            )
            .await?,
    ))
}
