use axum::RequestPartsExt;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum_extra::extract::PrivateCookieJar;
use kellnr_appstate::AppStateData;
use kellnr_settings::constants;


use crate::token;

#[derive(Debug, Clone)]
pub struct MaybeUser {
    pub name: String,
    pub is_admin: bool,
    pub is_read_only: bool,
}

impl MaybeUser {
    pub fn from_token(token: token::Token) -> Self {
        Self {
            name: token.user,
            is_admin: token.is_admin,
            is_read_only: token.is_read_only,
        }
    }

    pub fn from_session(name: String, is_admin: bool) -> Self {
        Self {
            name,
            is_admin,
            // Session auth does not currently expose read-only state.
            // Treat as not read-only for API actions.
            is_read_only: false,
        }
    }
}

impl FromRequestParts<AppStateData> for MaybeUser {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &AppStateData) -> Result<Self, Self::Rejection> {
        // 1) Prefer cargo token auth if present
        if let Ok(t) = token::Token::from_request_parts(parts, state).await {
            return Ok(Self::from_token(t));
        }

        // 2) Fallback to session cookie auth
        let jar: PrivateCookieJar = parts
            .extract_with_state(state)
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        let session_cookie = jar
            .get(constants::COOKIE_SESSION_ID)
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let (name, is_admin) = state
            .db
            .validate_session(session_cookie.value())
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        Ok(Self::from_session(name, is_admin))
    }
}
