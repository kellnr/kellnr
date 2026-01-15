use axum::RequestPartsExt;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use tracing::warn;

use crate::token::Token;

/// Middleware that checks if a cargo token is provided when `settings.registry.auth_required` is `true`.
///
/// If the user is not logged in, a 401 is returned.
///
/// Note: For endpoints that should accept both cargo tokens (CLI) and session cookies (Web UI),
/// use `token_or_session_auth_when_required` instead.
pub async fn cargo_auth_when_required(
    State(state): State<kellnr_appstate::AppStateData>,
    request: Request,
    next: Next,
) -> Response {
    // Do not expose publicly /config.json even if auth_required is true, cargo and other registries
    // are expected to retry with authentication if they got a 401 status code.
    // See:
    // - https://github.com/kellnr/kellnr/pull/773#discussion_r2300752458
    // - https://doc.rust-lang.org/cargo/reference/registry-index.html#sparse-authentication
    if !state.settings.registry.auth_required {
        // If auth_required is not true, pass through.
        return next.run(request).await;
    }

    let token = Token::from_header(request.headers(), &state.db).await;

    match token {
        Ok(_) => next.run(request).await,
        Err(status) => {
            // Forge the response to handle www-authenticate header.
            // See: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/WWW-Authenticate
            warn!("Authentication required, but failed: {status}");
            let mut response = Response::new(Body::empty());

            (*response.status_mut()) = status;
            response.headers_mut().insert(
                "WWW-Authenticate",
                HeaderValue::from_static("Basic, Bearer"),
            );

            response
        }
    }
}

/// Middleware that allows *either* cargo token auth (Authorization header) *or* web session auth
/// (signed session cookie) when `settings.registry.auth_required` is `true`.
///
/// This is intended for endpoints that are used by both the cargo CLI and the web UI.
pub async fn token_or_session_auth_when_required(
    State(state): State<kellnr_appstate::AppStateData>,
    request: Request,
    next: Next,
) -> Response {
    if !state.settings.registry.auth_required {
        return next.run(request).await;
    }

    // 1) Try cargo token auth.
    if Token::from_header(request.headers(), &state.db)
        .await
        .is_ok()
    {
        return next.run(request).await;
    }

    // 2) Try session cookie auth (signed cookie).
    // Note: extracting `PrivateCookieJar` from a full request consumes the request.
    // We therefore extract it from parts and then reconstruct the request.
    let (mut parts, body) = request.into_parts();

    let jar: axum_extra::extract::PrivateCookieJar = match parts.extract_with_state(&state).await {
        Ok(j) => j,
        Err(_) => return unauthorized_www_authenticate(),
    };

    let Some(cookie) = jar.get(kellnr_settings::constants::COOKIE_SESSION_ID) else {
        return unauthorized_www_authenticate();
    };

    if state.db.validate_session(cookie.value()).await.is_ok() {
        let request = Request::from_parts(parts, body);
        return next.run(request).await;
    }

    unauthorized_www_authenticate()
}

fn unauthorized_www_authenticate() -> Response {
    // Forge the response to handle www-authenticate header.
    // See: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/WWW-Authenticate
    let mut response = Response::new(Body::empty());
    *response.status_mut() = axum::http::StatusCode::UNAUTHORIZED;
    response.headers_mut().insert(
        "WWW-Authenticate",
        HeaderValue::from_static("Basic, Bearer"),
    );
    response
}

#[cfg(test)]
mod test {

    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::routing::get;
    use axum::{Router, middleware};
    use kellnr_appstate::AppStateData;
    use kellnr_db::User;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_settings::Settings;
    use mockall::predicate::*;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn no_auth_required() {
        let settings = test_settings(false);
        let r = app(settings)
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_required_but_not_provided() {
        let settings = test_settings(true);
        let r = app(settings)
            .oneshot(Request::get("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_required_but_wrong_token_provided() {
        let settings = test_settings(true);
        let r = app(settings)
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
        let settings = test_settings(true);
        let r = app(settings)
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

    pub async fn test_auth_req_token() -> StatusCode {
        StatusCode::OK
    }

    fn test_settings(auth_required: bool) -> Settings {
        Settings {
            registry: kellnr_settings::Registry {
                auth_required,
                ..kellnr_settings::Registry::default()
            },
            ..Settings::default()
        }
    }

    fn app(settings: Settings) -> Router {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("token"))
            .returning(move |_| {
                Ok(User {
                    id: 0,
                    name: "user".to_string(),
                    pwd: String::new(),
                    salt: String::new(),
                    is_admin: false,
                    is_read_only: false,
                })
            });
        mock_db
            .expect_get_user_from_token()
            .with(eq("wrong_token"))
            .returning(move |_| Err(DbError::UserNotFound("user".to_string())));

        println!("settings: {:?}", settings.registry);
        let state = AppStateData {
            db: Arc::new(mock_db),
            settings: Arc::new(settings),
            ..kellnr_appstate::test_state()
        };

        println!("Appstate registry: {:?}", state.settings.registry);

        Router::new()
            .route("/test", get(test_auth_req_token))
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                cargo_auth_when_required,
            ))
            .with_state(state)
    }
}

#[cfg(test)]
mod auth_middleware_tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use hyper::{Request, header};
    use kellnr_appstate::AppStateData;
    use kellnr_db::DbProvider;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_settings::Settings;
    use mockall::predicate::*;
    use tower::ServiceExt;

    use super::*;

    fn app_required_auth(db: Arc<dyn DbProvider>) -> Router {
        let settings = Settings::default();
        let state = AppStateData {
            db,
            settings: Arc::new(Settings {
                registry: kellnr_settings::Registry {
                    auth_required: true,
                    ..kellnr_settings::Registry::default()
                },
                ..settings
            }),
            ..kellnr_appstate::test_state()
        };

        Router::new()
            .route("/guarded", get(StatusCode::OK))
            .route_layer(from_fn_with_state(state.clone(), cargo_auth_when_required))
            .route("/not_guarded", get(StatusCode::OK))
            .with_state(state)
    }

    fn app_not_required_auth(db: Arc<dyn DbProvider>) -> Router {
        let settings = Settings::default();
        let state = AppStateData {
            db,
            settings: Arc::new(settings),
            ..kellnr_appstate::test_state()
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
            .oneshot(Request::get("/guarded").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn not_guarded_route_without_token() -> Result {
        let mock_db = MockDb::new();

        let r = app_required_auth(Arc::new(mock_db))
            .oneshot(Request::get("/not_guarded").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn app_not_required_auth_with_guarded_route() -> Result {
        let mock_db = MockDb::new();

        let r = app_not_required_auth(Arc::new(mock_db))
            .oneshot(Request::get("/guarded").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::OK);

        Ok(())
    }
}
