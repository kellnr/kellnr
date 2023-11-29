use crate::token::Token;
use appstate::AppStateData;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use tracing::warn;

// This token checks if "auth_required = true" and if so, it requires a token.
// Else, it does not require a token.
// Returns None if "auth_required = false", else returns Some(Token) or an error.
#[derive(Debug)]
pub struct AuthReqToken(Option<Token>);

#[axum::async_trait]
impl FromRequestParts<AppStateData> for AuthReqToken {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        if state.settings.registry.auth_required {
            Token::from_request_parts(parts, state)
                .await
                .map(|t| Self(Some(t)))
        } else {
            Ok(Self(None))
        }
    }
}

/// Middleware that checks if a cargo token is provided, when settings.registry.auth_required is true.<br>
/// If the user is not logged in, a 401 is returned.
pub async fn cargo_auth_when_required(
    State(state): State<appstate::AppStateData>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !state.settings.registry.auth_required {
        // If auth_required is not true, pass through.
        return Ok(next.run(request).await);
    }

    let token = Token::from_header(request.headers(), &state.db).await;

    match token {
        Ok(_) => Ok(next.run(request).await),
        Err(status) => {
            warn!("Authentication required, but failed: {}", status);
            Err(status)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use db::error::DbError;
    use db::mock::MockDb;
    use db::User;
    use mockall::predicate::*;
    use settings::Settings;
    use std::sync::Arc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn no_auth_required() {
        let settings = Settings {
            registry: settings::Registry {
                auth_required: false,
                ..settings::Registry::default()
            },
            ..Settings::default()
        };

        let r = app(settings)
            .await
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_required_but_not_provided() {
        let settings = Settings {
            registry: settings::Registry {
                auth_required: true,
                ..settings::Registry::default()
            },
            ..Settings::default()
        };

        let r = app(settings)
            .await
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_required_but_wrong_token_provided() {
        let settings = Settings {
            registry: settings::Registry {
                auth_required: true,
                ..settings::Registry::default()
            },
            ..Settings::default()
        };

        let r = app(settings)
            .await
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
            registry: settings::Registry {
                auth_required: true,
                ..settings::Registry::default()
            },
            ..Settings::default()
        };

        let r = app(settings)
            .await
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

        let state = AppStateData {
            db: Arc::new(mock_db),
            settings: Arc::new(settings),
            ..appstate::test_state().await
        };
        Router::new()
            .route("/test", get(test_auth_req_token))
            .with_state(state)
    }
}

#[cfg(test)]
mod auth_middleware_tests {
    use super::*;
    use appstate::AppStateData;
    use axum::body::Body;
    use axum::middleware::from_fn_with_state;
    use axum::{routing::get, Router};
    use db::DbProvider;
    use db::{error::DbError, mock::MockDb};
    use hyper::{header, Request};
    use mockall::predicate::*;
    use settings::Settings;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn app_required_auth(db: Arc<dyn DbProvider>) -> Router {
        let settings = Settings::default();
        let state = AppStateData {
            db,
            settings: Arc::new(Settings {
                registry: settings::Registry {
                    auth_required: true,
                    ..settings::Registry::default()
                },
                ..settings
            }),
            ..appstate::test_state().await
        };

        Router::new()
            .route("/guarded", get(StatusCode::OK))
            .route_layer(from_fn_with_state(state.clone(), cargo_auth_when_required))
            .route("/not_guarded", get(StatusCode::OK))
            .with_state(state)
    }

    async fn app_not_required_auth(db: Arc<dyn DbProvider>) -> Router {
        let settings = Settings::default();
        let state = AppStateData {
            db,
            settings: Arc::new(settings),
            ..appstate::test_state().await
        };
        Router::new()
            .route("/guarded", get(StatusCode::OK))
            .route_layer(from_fn_with_state(state.clone(), cargo_auth_when_required))
            .with_state(state)
    }

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn guarded_route_with_invalid_token() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("1234"))
            .returning(|_st| Err(DbError::UserNotFound("1234".to_owned())));

        let r = app_required_auth(Arc::new(mock_db))
            .await
            .oneshot(
                Request::get("/guarded")
                    .header(header::AUTHORIZATION, "1234")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);

        Ok(())
    }

    #[tokio::test]
    async fn guarded_route_without_token() -> Result {
        let mock_db = MockDb::new();

        let r = app_required_auth(Arc::new(mock_db))
            .await
            .oneshot(Request::get("/guarded").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn not_guarded_route_without_token() -> Result {
        let mock_db = MockDb::new();

        let r = app_required_auth(Arc::new(mock_db))
            .await
            .oneshot(Request::get("/not_guarded").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn app_not_required_auth_with_guarded_route() -> Result {
        let mock_db = MockDb::new();

        let r = app_not_required_auth(Arc::new(mock_db))
            .await
            .oneshot(Request::get("/guarded").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::OK);

        Ok(())
    }
}
