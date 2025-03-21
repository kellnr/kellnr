use appstate::AppStateData;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use db::DbProvider;
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};
use serde::Deserialize;
use std::iter;
use std::sync::Arc;

#[derive(Debug)]
pub struct Token {
    pub token: String,
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
    let mut rng = thread_rng();
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
        let token = headers
            .get("Authorization")
            .ok_or(StatusCode::UNAUTHORIZED)?
            .to_str()
            .map_err(|_| StatusCode::BAD_REQUEST)?
            .to_owned();

        let user = db
            .get_user_from_token(&token)
            .await
            .map_err(|_| StatusCode::FORBIDDEN)?;

        Ok(Token {
            token,
            user: user.name,
            is_admin: user.is_admin,
            is_read_only: user.is_read_only,
        })
    }
}

#[axum::async_trait]
impl FromRequestParts<AppStateData> for Token {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        Self::extract_token(&parts.headers, &state.db).await
    }
}

#[axum::async_trait]
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
