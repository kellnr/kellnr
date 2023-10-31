use appstate::AppStateData;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::Deserialize;
use std::iter;

#[derive(Debug)]
pub struct Token {
    pub token: String,
    pub user: String,
    pub is_admin: bool,
}

pub fn generate_token() -> String {
    let mut rng = thread_rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(32)
        .collect::<String>()
}

#[axum::async_trait]
impl FromRequestParts<AppStateData> for Token {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .ok_or_else(|| StatusCode::UNAUTHORIZED)?
            .to_str()
            .map_err(|_| StatusCode::BAD_REQUEST)?
            .to_owned();

        let user = state
            .db
            .get_user_from_token(&token)
            .await
            .map_err(|_| StatusCode::FORBIDDEN)?;

        Ok(Token {
            token,
            user: user.name,
            is_admin: user.is_admin,
        })
    }
}

#[derive(Deserialize)]
pub struct NewTokenReqData {
    pub name: String,
}
