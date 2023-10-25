use crate::token::Token;
use appstate::AppStateData;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;

// This token checks if "auth_required = true" and if so, it requires a token.
// Else, it does not require a token.
// Returns None if "auth_required = false", else returns Some(Token) or an error.
// Feature is only available in Enterprise version.
#[derive(Debug)]
pub struct AuthReqToken(Option<Token>);

#[axum::async_trait]
impl FromRequestParts<AppStateData> for AuthReqToken {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        if state.settings.auth_required {
            Token::from_request_parts(parts, state)
                .await
                .map(|t| Self(Some(t)))
        } else {
            Ok(Self(None))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use axum::Router;
    use axum::routing::get;
    use axum_extra::extract::cookie::Key;
    use db::error::DbError;
    use db::mock::MockDb;
    use db::User;
    use mockall::predicate::*;
    use settings::Settings;
    use std::sync::Arc;
    use tower::ServiceExt;
    use storage::kellnr_crate_storage::KellnrCrateStorage;

    #[tokio::test]
    async fn no_auth_required() {
        let settings = Settings {
            auth_required: false,
            ..Settings::new().unwrap()
        };

        let r = app(settings).await
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_required_but_not_provided() {
        let settings = Settings {
            auth_required: true,
            ..Settings::new().unwrap()
        };

        let r = app(settings).await
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_required_but_wrong_token_provided() {
        let settings = Settings {
            auth_required: true,
            ..Settings::new().unwrap()
        };

        let r = app(settings).await
            .oneshot(
                Request::get("/test")
                    .header(header::AUTHORIZATION, "wrong_token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn auth_required_and_right_token_provided() {
        let settings = Settings {
            auth_required: true,
            ..Settings::new().unwrap()
        };

        let r = app(settings).await
            .oneshot(
                Request::get("/test")
                    .header(header::AUTHORIZATION, "token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    pub async fn test_auth_req_token(auth_req_token: AuthReqToken) {
        _ = auth_req_token;
    }

    async fn app(settings: Settings) -> Router {
        const TEST_KEY: &[u8] = &[1; 64];
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("token"))
            .returning(move |_| {
                Ok(User {
                    id: 0,
                    name: "user".to_string(),
                    pwd: "".to_string(),
                    salt: "".to_string(),
                    is_admin: false,
                })
            });
        mock_db
            .expect_get_user_from_token()
            .with(eq("wrong_token"))
            .returning(move |_| Err(DbError::UserNotFound("user".to_string())));

        Router::new()
            .route("/test", get(test_auth_req_token))
            .with_state(AppStateData {
                db: Arc::new(mock_db),
                crate_storage: Arc::new(KellnrCrateStorage::new(&settings).await.unwrap()),
                settings: Arc::new(settings),
                signing_key: Key::from(TEST_KEY),
            })
    }
}
