use axum::RequestPartsExt;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::Cookie;
use cookie::{SameSite, time};
use kellnr_appstate::AppStateData;
use kellnr_common::util::generate_rand_string;
use kellnr_settings::constants::COOKIE_SESSION_ID;
use time::Duration;
use tracing::error;

use crate::error::RouteError;

/// Creates a new session for the user and returns a cookie jar with the session cookie set.
/// Generates a token, persists it via db, and adds the cookie using app_state settings.
pub(crate) async fn create_session_jar(
    cookies: PrivateCookieJar,
    app_state: &AppStateData,
    username: &str,
) -> Result<PrivateCookieJar, RouteError> {
    let session_token = generate_rand_string(12);
    app_state
        .db
        .add_session_token(username, &session_token)
        .await
        .map_err(|e| {
            error!("Failed to create session: {e}");
            RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;
    let session_age_seconds = app_state.settings.registry.session_age_seconds as i64;
    Ok(cookies.add(
        Cookie::build((COOKIE_SESSION_ID, session_token))
            .max_age(Duration::seconds(session_age_seconds))
            .same_site(SameSite::Strict)
            .path("/"),
    ))
}

pub trait Name {
    fn name(&self) -> String;
    fn new(name: String) -> Self;
}

pub struct AdminUser(pub String);

impl AdminUser {
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl Name for AdminUser {
    fn name(&self) -> String {
        self.0.clone()
    }
    fn new(name: String) -> Self {
        Self(name)
    }
}

impl axum::extract::FromRequestParts<AppStateData> for AdminUser {
    type Rejection = RouteError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let jar: PrivateCookieJar = parts.extract_with_state(state).await.unwrap();
        let session_cookie = jar.get(COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => match state.db.validate_session(cookie.value()).await {
                Ok((name, true)) => Ok(Self(name)),
                Ok((_, false)) => Err(RouteError::InsufficientPrivileges),
                Err(_) => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
            },
            None => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
        }
    }
}

pub struct NormalUser(pub String);
impl Name for NormalUser {
    fn name(&self) -> String {
        self.0.clone()
    }
    fn new(name: String) -> Self {
        Self(name)
    }
}

pub struct AnyUser(pub String);
impl Name for AnyUser {
    fn name(&self) -> String {
        self.0.clone()
    }
    fn new(name: String) -> Self {
        Self(name)
    }
}

#[derive(Debug)]
pub enum MaybeUser {
    // Consider using a db model or something?
    Normal(String),
    Admin(String),
}

impl MaybeUser {
    pub fn name(&self) -> &str {
        match self {
            Self::Normal(name) | Self::Admin(name) => name,
        }
    }

    pub fn assert_normal(&self) -> Result<(), RouteError> {
        match self {
            MaybeUser::Normal(_) => Ok(()),
            MaybeUser::Admin(_) => Err(RouteError::InsufficientPrivileges),
        }
    }

    pub fn assert_admin(&self) -> Result<(), RouteError> {
        match self {
            MaybeUser::Normal(_) => Err(RouteError::InsufficientPrivileges),
            MaybeUser::Admin(_) => Ok(()),
        }
    }
}

impl axum::extract::FromRequestParts<AppStateData> for MaybeUser {
    type Rejection = RouteError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let jar: PrivateCookieJar = parts.extract_with_state(state).await.unwrap();
        let session_cookie = jar.get(COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => match state.db.validate_session(cookie.value()).await {
                // admin
                Ok((name, true)) => Ok(Self::Admin(name)),
                // not admin
                Ok((name, false)) => Ok(Self::Normal(name)),
                Err(_) => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
            },
            None => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
        }
    }
}

impl axum::extract::OptionalFromRequestParts<AppStateData> for MaybeUser {
    type Rejection = RouteError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Option<Self>, Self::Rejection> {
        let jar: PrivateCookieJar = parts.extract_with_state(state).await.unwrap();
        let session_cookie = jar.get(COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => match state.db.validate_session(cookie.value()).await {
                // admin
                Ok((name, true)) => Ok(Some(Self::Admin(name))),
                // not admin
                Ok((name, false)) => Ok(Some(Self::Normal(name))),
                Err(_) => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
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
        Some(cookie) => match state.db.validate_session(cookie.value()).await {
            // user is logged in
            Ok(_) => Ok(next.run(request).await),
            // user is not logged in
            Err(_) => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
        },
        // user is not logged in
        None => Err(RouteError::Status(StatusCode::UNAUTHORIZED)),
    }
}

#[cfg(test)]
mod session_tests {
    use std::result;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::routing::get;
    use axum_extra::extract::cookie::Key;
    use hyper::{Request, StatusCode, header};
    use kellnr_appstate::AppStateData;
    use kellnr_db::DbProvider;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
    use mockall::predicate::*;
    use tower::ServiceExt;

    use super::*;
    use crate::test_helper::encode_cookies;

    async fn admin_endpoint(user: MaybeUser) -> result::Result<(), RouteError> {
        user.assert_admin()?;
        Ok(())
    }

    async fn normal_endpoint(user: MaybeUser) -> result::Result<(), RouteError> {
        user.assert_normal()?;
        Ok(())
    }

    async fn any_endpoint(_user: MaybeUser) {}

    fn app(db: Arc<dyn DbProvider>) -> Router {
        let settings = kellnr_settings::test_settings();
        let storage = Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
        Router::new()
            .route("/admin", get(admin_endpoint))
            .route("/normal", get(normal_endpoint))
            .route("/any", get(any_endpoint))
            .with_state(AppStateData {
                db,
                signing_key: Key::from(crate::test_helper::TEST_KEY),
                crate_storage: Arc::new(KellnrCrateStorage::new(&settings, storage)),
                settings: Arc::new(settings),
                ..kellnr_appstate::test_state()
            })
    }

    // AdminUser tests

    type Result<T = ()> = result::Result<T, Box<dyn std::error::Error>>;

    fn c1234() -> String {
        encode_cookies([(COOKIE_SESSION_ID, "1234")])
    }

    #[tokio::test]
    async fn admin_auth_works() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("admin".to_string(), true)));

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
            .with(eq("1234"))
            .returning(|_st| Ok(("admin".to_string(), false)));

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
            .with(eq("1234"))
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

    // NormalUser tests

    #[tokio::test]
    async fn normal_auth_works() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("normal".to_string(), false)));

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/normal")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn normal_auth_user_is_admin() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("normal".to_string(), true)));

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/normal")
                    .header(header::COOKIE, c1234())
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);

        Ok(())
    }

    #[tokio::test]
    async fn normal_auth_user_but_no_cookie_sent() -> Result {
        let mock_db = MockDb::new();

        let r = app(Arc::new(mock_db))
            .oneshot(Request::get("/normal").body(Body::empty())?)
            .await?;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn normal_auth_user_but_no_cookie_in_store() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Err(DbError::SessionNotFound));

        let r = app(Arc::new(mock_db))
            .oneshot(
                Request::get("/normal")
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
            .with(eq("1234"))
            .returning(|_st| Ok(("guest".to_string(), false)));

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
            .with(eq("1234"))
            .returning(|_st| Ok(("guest".to_string(), true)));

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
    async fn any_auth_user_but_no_cookie_in_store() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
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
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use axum_extra::extract::cookie::Key;
    use hyper::{Request, StatusCode, header};
    use kellnr_appstate::AppStateData;
    use kellnr_db::DbProvider;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_settings::Settings;
    use mockall::predicate::*;
    use tower::ServiceExt;

    use super::*;
    use crate::test_helper::encode_cookies;

    fn app_required_auth(db: Arc<dyn DbProvider>) -> Router {
        let settings = Settings::default();
        let state = AppStateData {
            db,
            signing_key: Key::from(crate::test_helper::TEST_KEY),
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
            signing_key: Key::from(crate::test_helper::TEST_KEY),
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
            .with(eq("1234"))
            .returning(|_st| Ok(("guest".to_string(), false)));

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
            .with(eq("1234"))
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
