use crate::{
    auth::SessionLease,
    http::AppState,
    vault::{self, KdfError},
};
use axum::{extract::State, http::StatusCode, Json};
use hmail_cleaner_protocol::browser::auth::{LoginRequest, SessionView};
use tower_cookies::{cookie::SameSite, Cookie, Cookies};
const SESSION_COOKIE: &str = "__Host-hmail-session";
pub(crate) fn lease(state: &AppState, cookies: &Cookies) -> Result<SessionLease, StatusCode> {
    let cookie = cookies
        .get(SESSION_COOKIE)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state
        .authority
        .authenticate(cookie.value())
        .map_err(|_| StatusCode::UNAUTHORIZED)
}
pub(crate) async fn login(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(input): Json<LoginRequest>,
) -> Result<Json<SessionView>, StatusCode> {
    if input.login.is_empty() || input.login.len() > 64 || input.password.len() > 4096 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !state.limiter.admit() {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let ticket = state
        .authority
        .begin_login()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let account = state
        .store
        .account()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    // Even a wrong login uses the same bounded verifier path; error details do not leave this boundary.
    let name_matches = input.login == account.login;
    let revision = account.revision;
    let dek = state
        .store
        .kdf
        .try_run(move || vault::open_password(&account.material, &input.password))
        .await
        .map_err(|error| match error {
            KdfError::Busy => StatusCode::TOO_MANY_REQUESTS,
            KdfError::Vault(_) => StatusCode::UNAUTHORIZED,
            KdfError::Worker => StatusCode::SERVICE_UNAVAILABLE,
        })?;
    if !name_matches {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let _barrier = state.authority.credential_barrier.lock().await;
    let current = state
        .store
        .account()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if current.revision != revision {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let grant = state
        .authority
        .admit_ticket(ticket, revision, dek)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let cookie = Cookie::build((SESSION_COOKIE, grant.token.as_str().to_owned()))
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .build();
    cookies.add(cookie);
    Ok(Json(SessionView {
        authenticated: true,
        generation: Some(grant.generation.into()),
    }))
}
pub(crate) async fn session(State(state): State<AppState>, cookies: Cookies) -> Json<SessionView> {
    let current = lease(&state, &cookies).ok();
    Json(SessionView {
        authenticated: current.is_some(),
        generation: current.map(|s| s.generation.into()),
    })
}
pub(crate) async fn logout(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(_): Json<serde_json::Value>,
) -> Result<StatusCode, StatusCode> {
    let current = lease(&state, &cookies)?;
    state.authority.revoke(current.generation);
    // No Set-Cookie deletion: a late response must not erase a newer generation's browser cookie.
    Ok(StatusCode::NO_CONTENT)
}
pub(crate) async fn activity(
    State(state): State<AppState>,
    cookies: Cookies,
    Json(_): Json<serde_json::Value>,
) -> Result<StatusCode, StatusCode> {
    let current = lease(&state, &cookies)?;
    state
        .authority
        .touch(current.generation)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok(StatusCode::NO_CONTENT)
}
