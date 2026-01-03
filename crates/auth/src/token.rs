use std::iter;
use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use kellnr_appstate::AppStateData;
use kellnr_db::DbProvider;
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use serde::Deserialize;

#[derive(Debug)]
pub struct Token {
    pub value: String,
    pub user: String,
    pub is_admin: bool,
    pub is_read_only: bool,
}

// See https://github.com/tokio-rs/axum/discussions/2281
#[derive(Debug)]
pub enum OptionToken {
    None,
    Some(Token),
}

pub fn generate_token() -> String {
    let mut rng = rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(32)
        .collect::<String>()
}

impl Token {
    pub async fn from_header(
        headers: &HeaderMap,
        db: &Arc<dyn DbProvider>,
    ) -> Result<Self, StatusCode> {
        Self::extract_token(headers, db).await
    }

    async fn extract_token(
        headers: &HeaderMap,
        db: &Arc<dyn DbProvider>,
    ) -> Result<Token, StatusCode> {
        // OptionToken code expects UNAUTHORIZED when no token is found
        let mut token = headers
            .get("Authorization")
            .ok_or(StatusCode::UNAUTHORIZED)?
            .to_str()
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        // Handle basic authentication
        if token.starts_with("Basic ") || token.starts_with("basic ") {
            let decoded = STANDARD
                .decode(&token[6..])
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            let decoded_str = String::from_utf8(decoded).map_err(|_| StatusCode::BAD_REQUEST)?;
            let (user, token) = decoded_str.split_once(':').ok_or(StatusCode::BAD_REQUEST)?;

            let user = db.get_user(user).await.map_err(|_| StatusCode::FORBIDDEN)?;
            if db.authenticate_user(&user.name, token).await.is_err() {
                return Err(StatusCode::FORBIDDEN);
            }

            return Ok(Token {
                value: token.to_string(),
                user: user.name,
                is_admin: user.is_admin,
                is_read_only: user.is_read_only,
            });
        }

        // Handle bearer authentication
        if token.starts_with("Bearer ") || token.starts_with("bearer ") {
            token = &token[7..];
        }

        let user = db
            .get_user_from_token(token)
            .await
            .map_err(|_| StatusCode::FORBIDDEN)?;

        Ok(Token {
            value: token.to_string(),
            user: user.name,
            is_admin: user.is_admin,
            is_read_only: user.is_read_only,
        })
    }
}

impl FromRequestParts<AppStateData> for Token {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        Self::extract_token(&parts.headers, &state.db).await
    }
}

impl FromRequestParts<AppStateData> for OptionToken {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        match Token::extract_token(&parts.headers, &state.db).await {
            Ok(token) => Ok(OptionToken::Some(token)),
            Err(StatusCode::UNAUTHORIZED) => Ok(OptionToken::None),
            Err(status_code) => Err(status_code),
        }
    }
}

#[derive(Deserialize)]
pub struct NewTokenReqData {
    pub name: String,
}
