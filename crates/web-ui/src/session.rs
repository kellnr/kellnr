use axum::RequestPartsExt;
use axum::extract::{FromRequestParts, OptionalFromRequestParts, Request, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::Cookie;
use cookie::{SameSite, time};
use kellnr_appstate::AppStateData;
use kellnr_common::account_name::UserName;
use kellnr_common::plain_token::PlainToken;
use kellnr_common::util::generate_rand_string;
use kellnr_db::SessionInfo;
use kellnr_db::error::DbError;
use kellnr_settings::constants::COOKIE_SESSION_ID;
use time::Duration;
use tracing::error;

use crate::error::RouteError;

/// Creates a new session for the user and returns a cookie jar with the session cookie set.
/// Generates a token, persists it via db, and adds the cookie using `app_state` settings.
pub(crate) async fn create_session_jar(
    cookies: PrivateCookieJar,
    app_state: &AppStateData,
    username: &UserName,
) -> Result<PrivateCookieJar, RouteError> {
    // 32 alphanumeric chars (~190 bits) to match the API token length, well
    // above the ~128-bit session-id recommendation.
    let session_token = PlainToken::from(generate_rand_string(32));
    app_state
        .db
        .add_session_token(username, &session_token)
        .await
        .map_err(|e| {
            error!("Failed to create session: {e}");
            RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;
    // `Secure` follows `origin.protocol` unless `origin.cookie_secure` forces
    // it, since browsers drop secure cookies on plain-HTTP deployments.
    let secure = app_state.settings.origin.secure_cookies();
    Ok(cookies.add(session_cookie(
        COOKIE_SESSION_ID,
        session_token.into_secret(),
        app_state.settings.registry.session_age().cookie_max_age(),
        secure,
    )))
}

pub(crate) fn session_cookie(
    name: &'static str,
    value: String,
    max_age: std::time::Duration,
    secure: bool,
) -> Cookie<'static> {
    // `max_age` is bounded by `MAX_SESSION_AGE`, which always fits.
    let max_age = Duration::try_from(max_age).unwrap_or(Duration::MAX);
    Cookie::build((name, value))
        .max_age(max_age)
        .same_site(SameSite::Strict)
        .http_only(true)
        .secure(secure)
        .path("/")
        .into()
}

/// Looks up the session of `session_token`. A missing or expired session is
/// a 401, any other failure a 500.
async fn validate_session(
    state: &AppStateData,
    session_token: &str,
) -> Result<SessionInfo, RouteError> {
    state
        .db
        .validate_session(&PlainToken::from(session_token))
        .await
        .map_err(|e| match e {
            DbError::SessionNotFound => RouteError::Status(StatusCode::UNAUTHORIZED),
            e => {
                error!("Failed to validate session: {e}");
                RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
            }
        })
}

pub struct AdminUser(pub UserName);

impl AdminUser {
    pub fn name(&self) -> &UserName {
        &self.0
    }
}

impl FromRequestParts<AppStateData> for AdminUser {
    type Rejection = RouteError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let jar: PrivateCookieJar = parts.extract_with_state(state).await.unwrap();
        let session_cookie = jar.get(COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => match validate_session(state, cookie.value()).await? {
                session if session.is_admin => Ok(Self(session.name)),
                _ => Err(RouteError::InsufficientPrivileges),
            },
            None => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
        }
    }
}

#[derive(Debug)]
pub enum MaybeUser {
    Normal(UserName),
    Admin(UserName),
}

impl MaybeUser {
    pub fn name(&self) -> &UserName {
        match self {
            Self::Normal(name) | Self::Admin(name) => name,
        }
    }
}

impl FromRequestParts<AppStateData> for MaybeUser {
    type Rejection = RouteError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let jar: PrivateCookieJar = parts.extract_with_state(state).await.unwrap();
        let session_cookie = jar.get(COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => match validate_session(state, cookie.value()).await? {
                session if session.is_admin => Ok(Self::Admin(session.name)),
                session => Ok(Self::Normal(session.name)),
            },
            None => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
        }
    }
}

impl OptionalFromRequestParts<AppStateData> for MaybeUser {
    type Rejection = RouteError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Option<Self>, Self::Rejection> {
        let jar: PrivateCookieJar = parts.extract_with_state(state).await.unwrap();
        let session_cookie = jar.get(COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => match validate_session(state, cookie.value()).await? {
                session if session.is_admin => Ok(Some(Self::Admin(session.name))),
                session => Ok(Some(Self::Normal(session.name))),
            },
            None => Ok(None),
        }
    }
}

/// Middleware that checks if a user is logged in when `settings.registry.auth_required` is `true`
/// If the user is not logged in, a 401 is returned.
pub async fn session_auth_when_required(
    State(state): State<AppStateData>,
    jar: PrivateCookieJar,
    request: Request,
    next: Next,
) -> Result<Response, RouteError> {
    if !state.settings.registry.auth_required {
        // If "auth_required" is "false", pass through.
        return Ok(next.run(request).await);
    }
    let session_cookie = jar.get(COOKIE_SESSION_ID);
    match session_cookie {
        Some(cookie) => {
            validate_session(&state, cookie.value()).await?;
            Ok(next.run(request).await)
        }
        // user is not logged in
        None => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
    }
}

#[cfg(test)]
mod session_tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::header;
    use axum::routing::get;
    use cookie::Key;
    use kellnr_appstate::encode_cookies;
    use kellnr_db::DbProvider;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
    use mockall::predicate::eq;
    use tower::ServiceExt;

    use super::*;

    type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

    async fn admin_endpoint(_user: AdminUser) {}

    async fn any_endpoint(_user: MaybeUser) {}

    fn app(db: Arc<dyn DbProvider>) -> Router {
        let settings = kellnr_settings::test_settings();
        let storage = Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
        Router::new()
            .route("/admin", get(admin_endpoint))
            .route("/any", get(any_endpoint))
            .with_state(AppStateData {
                db,
                signing_key: Key::from(kellnr_appstate::TEST_KEY),
                crate_storage: Arc::new(KellnrCrateStorage::new(&settings, storage)),
                settings: Arc::new(settings),
                ..kellnr_appstate::test_state()
            })
    }

    // AdminUser tests

    fn c1234() -> String {
        encode_cookies([(COOKIE_SESSION_ID, "1234")])
    }

    #[tokio::test]
    async fn admin_auth_works() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| {
                Ok(SessionInfo {
                    name: UserName::new("admin").unwrap(),
                    is_admin: true,
                    is_read_only: false,
                })
            });

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/admin")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "1234")]),
                    )
                    .body(Body::empty())?,
            )
            .await?;
        assert!(r.status().is_success());

        Ok(())
    }

    #[tokio::test]
    async fn admin_auth_user_is_no_admin() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| {
                Ok(SessionInfo {
                    name: UserName::new("admin").unwrap(),
                    is_admin: false,
                    is_read_only: false,
                })
            });

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/admin")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);

        Ok(())
    }

    #[tokio::test]
    async fn admin_auth_user_but_no_cookie_sent() -> Result {
        let mock_db = MockDb::new();

        let r = app(Arc::new(mock_db))
            .oneshot(Request::get("/admin").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn admin_auth_user_but_no_cookie_in_store() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| Err(DbError::SessionNotFound));

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/admin")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    // Guest User tests

    #[tokio::test]
    async fn any_auth_user_is_normal() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| {
                Ok(SessionInfo {
                    name: UserName::new("guest").unwrap(),
                    is_admin: false,
                    is_read_only: false,
                })
            });

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/any")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn any_auth_user_is_admin() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| {
                Ok(SessionInfo {
                    name: UserName::new("guest").unwrap(),
                    is_admin: true,
                    is_read_only: false,
                })
            });

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/any")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn any_auth_user_but_no_cookie_sent() -> Result {
        let mock_db = MockDb::new();

        let r = app(Arc::new(mock_db))
            .oneshot(Request::get("/any").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn any_auth_database_error() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| Err(DbError::FailedToCountCrates));

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/any")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(r.status(), StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn any_auth_user_but_no_cookie_in_store() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| Err(DbError::SessionNotFound));

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/any")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}

#[cfg(test)]
mod auth_middleware_tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::header;
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use cookie::Key;
    use kellnr_appstate::encode_cookies;
    use kellnr_db::DbProvider;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_settings::Settings;
    use mockall::predicate::eq;
    use tower::ServiceExt;

    use super::*;

    fn app_required_auth(db: Arc<dyn DbProvider>) -> Router {
        let settings = Settings::default();
        let state = AppStateData {
            db,
            signing_key: Key::from(kellnr_appstate::TEST_KEY),
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
            .route_layer(from_fn_with_state(
                state.clone(),
                session_auth_when_required,
            ))
            .route("/not_guarded", get(StatusCode::OK))
            .with_state(state)
    }

    fn app_not_required_auth(db: Arc<dyn DbProvider>) -> Router {
        let settings = Settings::default();
        let state = AppStateData {
            db,
            signing_key: Key::from(kellnr_appstate::TEST_KEY),
            settings: Arc::new(settings),
            ..kellnr_appstate::test_state()
        };
        Router::new()
            .route("/guarded", get(StatusCode::OK))
            .route_layer(from_fn_with_state(
                state.clone(),
                session_auth_when_required,
            ))
            .with_state(state)
    }

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    fn c1234() -> String {
        encode_cookies([(COOKIE_SESSION_ID, "1234")])
    }

    #[tokio::test]
    async fn guarded_route_with_valid_cookie() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| {
                Ok(SessionInfo {
                    name: UserName::new("guest").unwrap(),
                    is_admin: false,
                    is_read_only: false,
                })
            });

        let r = app_required_auth(Arc::new(mock_db))
            .oneshot(
                Request::get("/guarded")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn guarded_route_with_invalid_cookie() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| Err(DbError::SessionNotFound));

        let r = app_required_auth(Arc::new(mock_db))
            .oneshot(
                Request::get("/guarded")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    /// A failing database is not a missing login.
    #[tokio::test]
    async fn guarded_route_with_database_error() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(|_st| Err(DbError::FailedToCountCrates));

        let r = app_required_auth(Arc::new(mock_db))
            .oneshot(
                Request::get("/guarded")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::INTERNAL_SERVER_ERROR);

        Ok(())
    }

    #[tokio::test]
    async fn guarded_route_without_cookie() -> Result {
        let mock_db = MockDb::new();

        let r = app_required_auth(Arc::new(mock_db))
            .oneshot(Request::get("/guarded").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn not_guarded_route_without_cookie() -> Result {
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
mod session_cookie_tests {
    use std::sync::Arc;

    use axum::http::HeaderMap;
    use axum_extra::extract::cookie::Key;
    use kellnr_db::mock::MockDb;
    use kellnr_settings::constants::MAX_SESSION_AGE;
    use kellnr_settings::{Protocol, Settings};

    use super::*;

    async fn new_session_cookie(settings: Settings) -> Cookie<'static> {
        let mut db = MockDb::new();
        db.expect_add_session_token().returning(|_, _| Ok(()));
        let state = AppStateData {
            db: Arc::new(db),
            settings: Arc::new(settings),
            ..kellnr_appstate::test_state()
        };
        let jar = PrivateCookieJar::from_headers(&HeaderMap::new(), Key::generate());

        let jar = create_session_jar(jar, &state, &UserName::new("user").unwrap())
            .await
            .unwrap();

        jar.get(COOKIE_SESSION_ID).unwrap()
    }

    async fn session_cookie_secure(protocol: Protocol, cookie_secure: Option<bool>) -> bool {
        let mut settings = Settings::clone(&kellnr_appstate::test_state().settings);
        settings.origin.protocol = protocol;
        settings.origin.cookie_secure = cookie_secure;
        new_session_cookie(settings).await.secure().unwrap_or(false)
    }

    async fn session_cookie_max_age(session_age_seconds: u64) -> Option<Duration> {
        let mut settings = Settings::clone(&kellnr_appstate::test_state().settings);
        settings.registry.session_age_seconds = session_age_seconds;
        new_session_cookie(settings).await.max_age()
    }

    #[tokio::test]
    async fn cookie_lives_as_long_as_the_session() {
        assert_eq!(
            session_cookie_max_age(3600).await,
            Some(Duration::seconds(3600))
        );
    }

    /// A session age of zero means sessions never expire. A cookie with
    /// `Max-Age=0` would be dropped by the browser right away.
    #[tokio::test]
    async fn cookie_of_a_session_that_never_expires_is_kept() {
        let max_age = Duration::try_from(MAX_SESSION_AGE).unwrap();
        assert_eq!(session_cookie_max_age(0).await, Some(max_age));
        assert_eq!(session_cookie_max_age(u64::MAX).await, Some(max_age));
    }

    #[tokio::test]
    async fn secure_follows_protocol_by_default() {
        assert!(session_cookie_secure(Protocol::Https, None).await);
        assert!(!session_cookie_secure(Protocol::Http, None).await);
    }

    /// Behind a TLS-terminating proxy the configured protocol is http, but
    /// the operator can still force the `Secure` flag.
    #[tokio::test]
    async fn cookie_secure_setting_overrides_protocol() {
        assert!(session_cookie_secure(Protocol::Http, Some(true)).await);
        assert!(!session_cookie_secure(Protocol::Https, Some(false)).await);
    }
}
