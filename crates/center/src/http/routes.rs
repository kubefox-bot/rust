use super::AppState;
use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Router,
};
use tower_cookies::CookieManagerLayer;
use tower_http::csrf::CsrfLayer;
const LOGIN_BODY_BYTES: usize = 8192;
pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/auth/login", post(crate::auth::http::login))
        .route("/auth/logout", post(crate::auth::http::logout))
        .route("/auth/activity", post(crate::auth::http::activity))
        .route("/auth/session", get(crate::auth::http::session))
        .route(
            "/settings/session",
            get(super::settings::read).patch(super::settings::update),
        )
        .merge(crate::profiles::http::routes())
        .merge(crate::agents::http::routes())
        .merge(crate::jobs::http::routes())
        .merge(crate::history::http::routes())
        .layer(DefaultBodyLimit::max(LOGIN_BODY_BYTES))
        .layer(CookieManagerLayer::new())
        .layer(CsrfLayer::new())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            super::csrf::enforce,
        ))
        .layer(middleware::from_fn(super::csrf::deadline))
        .layer(middleware::from_fn(super::csrf::no_store));
    Router::new()
        .nest("/api/v1", api)
        .route("/healthz", get(|| async { "ok" }))
        .with_state(state)
}
