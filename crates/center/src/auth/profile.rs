//! Encrypted display metadata. The profile cookie never authenticates a session.
mod cipher;
#[cfg(test)]
mod tests;
use axum::http::StatusCode;
use cipher::ProfileCipher;
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tower_cookies::{cookie::SameSite, Cookie, Cookies};
use zeroize::Zeroizing;

pub(crate) const PROFILE_COOKIE: &str = "__Host-sochi-profile";
const PROFILE_TTL_SECONDS: u64 = 300;
#[derive(Clone)]
pub(crate) struct ProfileTokens {
    cipher: ProfileCipher,
    login: Arc<str>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileClaims {
    sub: String,
    name: String,
    login: String,
    role: String,
    sid: u64,
    iat: u64,
    exp: u64,
}
impl ProfileTokens {
    pub(crate) fn new(login: String) -> Result<Self, StatusCode> {
        Ok(Self {
            cipher: ProfileCipher::new().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
            login: login.into(),
        })
    }
    fn token_at(&self, generation: u64, issued: u64) -> Result<String, StatusCode> {
        let claims = ProfileClaims {
            sub: "1".into(),
            name: "Суперадмин".into(),
            login: self.login.to_string(),
            role: "superadmin".into(),
            sid: generation,
            iat: issued,
            exp: issued
                .checked_add(PROFILE_TTL_SECONDS)
                .ok_or(StatusCode::SERVICE_UNAVAILABLE)?,
        };
        let plaintext = Zeroizing::new(
            serde_json::to_vec(&claims).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        );
        self.cipher
            .seal(&plaintext)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
    }
    fn accepts_at(&self, token: &str, generation: u64, now: u64) -> bool {
        let Ok(plaintext) = self.cipher.open(token) else {
            return false;
        };
        let Ok(claims) = serde_json::from_slice::<ProfileClaims>(&plaintext) else {
            return false;
        };
        claims.sub == "1"
            && claims.name == "Суперадмин"
            && claims.login == self.login.as_ref()
            && claims.role == "superadmin"
            && claims.sid == generation
            && claims.iat <= now
            && claims.exp > now
            && claims.exp.checked_sub(claims.iat) == Some(PROFILE_TTL_SECONDS)
    }
    pub(crate) fn accepts(&self, token: &str, generation: u64) -> bool {
        now().is_ok_and(|now| self.accepts_at(token, generation, now))
    }
    pub(crate) fn issue(&self, cookies: &Cookies, generation: u64) -> Result<(), StatusCode> {
        cookies.add(
            Cookie::build((PROFILE_COOKIE, self.token_at(generation, now()?)?))
                .secure(true)
                .http_only(true)
                .same_site(SameSite::Strict)
                .path("/")
                .build(),
        );
        Ok(())
    }
}
fn now() -> Result<u64, StatusCode> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|now| now.as_secs())
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
pub(super) fn remove(cookies: &Cookies) {
    cookies.remove(
        Cookie::build((PROFILE_COOKIE, ""))
            .secure(true)
            .http_only(true)
            .same_site(SameSite::Strict)
            .path("/")
            .build(),
    );
}
