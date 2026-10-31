use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use tracing::warn;

use crate::identity::{Accepts, identify};
use crate::state::AppStateData;

/// Middleware that rejects requests without a login of one of the accepted
/// methods when `settings.registry.auth_required` is `true`.
///
/// Endpoints for cargo answer with a `WWW-Authenticate` header. Cargo and
/// other registry clients retry with credentials after a 401, so `/config.json`
/// is not public even if `auth_required` is `true`. See:
/// - <https://github.com/kellnr/kellnr/pull/773#discussion_r2300752458>
/// - <https://doc.rust-lang.org/cargo/reference/registry-index.html#sparse-authentication>
///
/// If both methods are accepted, every failed login is a 401.
pub async fn auth_when_required(
    State((state, accepts)): State<(AppStateData, Accepts)>,
    request: Request,
    next: Next,
) -> Response {
    if !state.settings.registry.auth_required {
        return next.run(request).await;
    }

    let (mut parts, body) = request.into_parts();
    match identify(&mut parts, &state, accepts).await {
        Ok(_) => next.run(Request::from_parts(parts, body)).await,
        Err(status) => {
            warn!("Authentication required, but failed: {status}");
            rejection(status, accepts)
        }
    }
}

fn rejection(status: StatusCode, accepts: Accepts) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = match accepts {
        Accepts::TokenOrSession => StatusCode::UNAUTHORIZED,
        Accepts::Token | Accepts::Session => status,
    };
    if accepts != Accepts::Session {
        // See: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/WWW-Authenticate
        response.headers_mut().insert(
            "WWW-Authenticate",
            HeaderValue::from_static("Basic, Bearer"),
        );
    }
    response
}

#[cfg(test)]
mod test {

    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::routing::get;
    use axum::{Router, middleware};
    use crate::state::AppStateData;
    use kellnr_common::account_name::UserName;
    use kellnr_common::plain_token::PlainToken;
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

    pub async fn ok_handler() -> StatusCode {
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
            .with(eq(PlainToken::from("token")))
            .returning(move |_| {
                Ok(User {
                    id: kellnr_db::UserId::new(0),
                    name: UserName::new("user").unwrap(),
                    pwd: String::new(),
                    salt: String::new(),
                    is_admin: false,
                    is_read_only: false,
                    created: String::new(),
                })
            });
        mock_db
            .expect_get_user_from_token()
            .with(eq(PlainToken::from("wrong_token")))
            .returning(move |_| Err(DbError::UserNotFound("user".to_string())));

        let state = AppStateData {
            db: Arc::new(mock_db),
            settings: Arc::new(settings),
            ..crate::test_support::test_state()
        };

        Router::new()
            .route("/test", get(ok_handler))
            .route_layer(middleware::from_fn_with_state(
                (state.clone(), Accepts::Token),
                auth_when_required,
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
    use crate::state::AppStateData;
    use kellnr_common::plain_token::PlainToken;
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
            ..crate::test_support::test_state()
        };

        Router::new()
            .route("/guarded", get(StatusCode::OK))
            .route_layer(from_fn_with_state((state.clone(), Accepts::Token), auth_when_required))
            .route("/not_guarded", get(StatusCode::OK))
            .with_state(state)
    }

    fn app_not_required_auth(db: Arc<dyn DbProvider>) -> Router {
        let settings = Settings::default();
        let state = AppStateData {
            db,
            settings: Arc::new(settings),
            ..crate::test_support::test_state()
        };
        Router::new()
            .route("/guarded", get(StatusCode::OK))
            .route_layer(from_fn_with_state((state.clone(), Accepts::Token), auth_when_required))
            .with_state(state)
    }

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn guarded_route_with_invalid_token() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq(PlainToken::from("1234")))
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

#[cfg(test)]
mod session_and_combined_tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::header;
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use cookie::Key;
    use kellnr_common::account_name::UserName;
    use kellnr_common::plain_token::PlainToken;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{DbProvider, SessionInfo};
    use kellnr_settings::Settings;
    use kellnr_settings::constants::COOKIE_SESSION_ID;
    use mockall::predicate::eq;
    use tower::ServiceExt;

    use super::*;
    use crate::test_support::{TEST_KEY, encode_cookies, test_state};

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    fn app(db: MockDb, auth_required: bool, accepts: Accepts) -> Router {
        let db: Arc<dyn DbProvider> = Arc::new(db);
        let state = AppStateData {
            db,
            signing_key: Key::from(TEST_KEY),
            settings: Arc::new(Settings {
                registry: kellnr_settings::Registry {
                    auth_required,
                    ..kellnr_settings::Registry::default()
                },
                ..Settings::default()
            }),
            ..test_state()
        };
        Router::new()
            .route("/guarded", get(StatusCode::OK))
            .route_layer(from_fn_with_state((state.clone(), accepts), auth_when_required))
            .route("/not_guarded", get(StatusCode::OK))
            .with_state(state)
    }

    fn session_db(result: fn() -> kellnr_db::provider::DbResult<SessionInfo>) -> MockDb {
        let mut db = MockDb::new();
        db.expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(move |_| result());
        db.expect_get_user_from_token()
            .returning(|_| Err(DbError::TokenNotFound));
        db
    }

    fn valid_session() -> kellnr_db::provider::DbResult<SessionInfo> {
        Ok(SessionInfo {
            name: UserName::new("guest").unwrap(),
            is_admin: false,
            is_read_only: false,
        })
    }

    async fn send(app: Router, uri: &str, cookie: bool) -> Result<Response> {
        let mut request = Request::get(uri);
        if cookie {
            request = request.header(header::COOKIE, encode_cookies([(COOKIE_SESSION_ID, "1234")]));
        }
        Ok(app.oneshot(request.body(Body::empty())?).await?)
    }

    #[tokio::test]
    async fn guarded_route_with_valid_cookie() -> Result {
        let r = send(app(session_db(valid_session), true, Accepts::Session), "/guarded", true).await?;
        assert_eq!(r.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn guarded_route_with_invalid_cookie() -> Result {
        let db = session_db(|| Err(DbError::SessionNotFound));
        let r = send(app(db, true, Accepts::Session), "/guarded", true).await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        assert!(!r.headers().contains_key("WWW-Authenticate"));
        Ok(())
    }

    /// A failing database is not a missing login.
    #[tokio::test]
    async fn guarded_route_with_database_error() -> Result {
        let db = session_db(|| Err(DbError::FailedToCountCrates));
        let r = send(app(db, true, Accepts::Session), "/guarded", true).await?;
        assert_eq!(r.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn guarded_route_without_cookie() -> Result {
        let r = send(app(MockDb::new(), true, Accepts::Session), "/guarded", false).await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn not_guarded_route_without_cookie() -> Result {
        let r = send(app(MockDb::new(), true, Accepts::Session), "/not_guarded", false).await?;
        assert_eq!(r.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn app_not_required_auth_with_guarded_route() -> Result {
        let r = send(app(MockDb::new(), false, Accepts::Session), "/guarded", false).await?;
        assert_eq!(r.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn token_or_session_accepts_session() -> Result {
        let r = send(app(session_db(valid_session), true, Accepts::TokenOrSession), "/guarded", true)
            .await?;
        assert_eq!(r.status(), StatusCode::OK);
        Ok(())
    }

    /// Cargo retries with credentials after a 401, so every failed login is
    /// one, also a failing database.
    #[tokio::test]
    async fn token_or_session_answers_every_failure_with_401() -> Result {
        let db = session_db(|| Err(DbError::FailedToCountCrates));
        let r = send(app(db, true, Accepts::TokenOrSession), "/guarded", true).await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(r.headers()["WWW-Authenticate"], "Basic, Bearer");
        Ok(())
    }
}
