use super::{static_files, AppState};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use tower_cookies::Cookies;

#[derive(Clone, Copy)]
pub(super) struct HeaderProfile(pub(super) bool);

pub(super) async fn populate(
    State(state): State<AppState>,
    cookies: Cookies,
    mut request: Request,
    next: Next,
) -> Response {
    if static_files::is_page(request.uri().path()) {
        // A display cookie alone is never evidence of a live session.
        let current = crate::auth::http::lease(&state, &cookies).ok();
        let authenticated = current.as_ref().is_some_and(|lease| {
            let valid = cookies
                .get(crate::auth::profile::PROFILE_COOKIE)
                .is_some_and(|cookie| {
                    state
                        .profile_tokens
                        .accepts(cookie.value(), lease.generation())
                });
            let ready = valid
                || state
                    .profile_tokens
                    .issue(&cookies, lease.generation())
                    .is_ok();
            ready && lease.is_live()
        });
        request
            .extensions_mut()
            .insert(HeaderProfile(authenticated));
    }
    next.run(request).await
}
