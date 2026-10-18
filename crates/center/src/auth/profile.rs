//! Signed display metadata. This JWT is never an authentication credential.
use axum::http::StatusCode;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Serialize;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tower_cookies::{cookie::SameSite, Cookie, Cookies};

pub(super) const PROFILE_COOKIE: &str = "__Host-sochi-profile";
const PROFILE_TTL_SECONDS: u64 = 300;
const SIGNING_KEY_BYTES: usize = 32;

#[derive(Clone)]
pub(crate) struct ProfileTokens {
    key: Arc<EncodingKey>,
    login: Arc<str>,
}
#[derive(Clone, Serialize)]
struct ProfileClaims {
    sub: &'static str,
    name: &'static str,
    login: String,
    role: &'static str,
    iss: &'static str,
    aud: &'static str,
    sid: String,
    iat: u64,
    exp: u64,
}
impl ProfileTokens {
    pub(crate) fn new(login: String) -> Result<Self, StatusCode> {
        let mut secret = zeroize::Zeroizing::new([0; SIGNING_KEY_BYTES]);
        SystemRandom::new()
            .fill(&mut *secret)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        Ok(Self {
            key: Arc::new(EncodingKey::from_secret(&*secret)),
            login: login.into(),
        })
    }
    fn token(&self, generation: u64) -> Result<String, StatusCode> {
        let issued = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .as_secs();
        let claims = ProfileClaims {
            sub: "1", // The MVP account is the singleton account row, not a session ID.
            name: "Суперадмин",
            login: self.login.to_string(),
            role: "superadmin",
            iss: "sochi-center",
            aud: "header-display",
            sid: generation.to_string(),
            iat: issued,
            exp: issued + PROFILE_TTL_SECONDS,
        };
        encode(&Header::new(Algorithm::HS256), &claims, &self.key)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
    }
    pub(crate) fn issue(&self, cookies: &Cookies, generation: u64) -> Result<(), StatusCode> {
        cookies.add(
            Cookie::build((PROFILE_COOKIE, self.token(generation)?))
                .secure(true)
                .http_only(false)
                .same_site(SameSite::Strict)
                .path("/")
                .build(),
        );
        Ok(())
    }
}

pub(super) fn remove(cookies: &Cookies) {
    cookies.remove(
        Cookie::build((PROFILE_COOKIE, ""))
            .secure(true)
            .same_site(SameSite::Strict)
            .path("/")
            .build(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{decode, DecodingKey, Validation};

    #[test]
    fn display_token_is_signed_scoped_and_contains_no_session_credential() {
        let secret = [7u8; SIGNING_KEY_BYTES];
        let tokens = ProfileTokens {
            key: Arc::new(EncodingKey::from_secret(&secret)),
            login: "operator".into(),
        };
        let token = tokens.token(42).unwrap();
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&["sochi-center"]);
        validation.set_audience(&["header-display"]);
        let verified =
            decode::<serde_json::Value>(&token, &DecodingKey::from_secret(&secret), &validation)
                .unwrap();
        assert_eq!(verified.claims["sub"], "1");
        assert_eq!(verified.claims["login"], "operator");
        assert_eq!(verified.claims["role"], "superadmin");
        assert_eq!(verified.claims["sid"], "42");
        assert_eq!(
            verified.claims["exp"].as_u64().unwrap() - verified.claims["iat"].as_u64().unwrap(),
            PROFILE_TTL_SECONDS
        );
        assert!(verified.claims.get("token").is_none());
        assert!(decode::<serde_json::Value>(
            &token,
            &DecodingKey::from_secret(&[8u8; SIGNING_KEY_BYTES]),
            &validation
        )
        .is_err());
    }
}
