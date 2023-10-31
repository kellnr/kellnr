use crate::error::RouteError;
use axum::http::request::Parts;
use axum::RequestPartsExt;
use axum_extra::extract::PrivateCookieJar;
use settings::constants;

pub trait Name {
    fn name(&self) -> String;
    fn new(name: String) -> Self;
}

pub struct AdminUser(pub String);
impl Name for AdminUser {
    fn name(&self) -> String {
        self.0.to_owned()
    }
    fn new(name: String) -> Self {
        Self(name)
    }
}

pub struct NormalUser(pub String);
impl Name for NormalUser {
    fn name(&self) -> String {
        self.0.to_owned()
    }
    fn new(name: String) -> Self {
        Self(name)
    }
}

pub struct AnyUser(pub String);
impl Name for AnyUser {
    fn name(&self) -> String {
        self.0.to_owned()
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

#[axum::async_trait]
impl axum::extract::FromRequestParts<appstate::AppStateData> for MaybeUser {
    type Rejection = RouteError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &appstate::AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let jar: PrivateCookieJar = parts.extract_with_state(state).await.unwrap();
        let session_cookie = jar.get(constants::COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => match state.db.validate_session(cookie.value()).await {
                // admin
                Ok((name, true)) => Ok(Self::Admin(name)),
                // not admin
                Ok((name, false)) => Ok(Self::Normal(name)),
                Err(_) => Err(RouteError::Status(axum::http::StatusCode::UNAUTHORIZED)),
            },
            None => Err(RouteError::Status(axum::http::StatusCode::UNAUTHORIZED)),
        }
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;
    use crate::test_helper::encode_cookies;
    use appstate::AppStateData;
    use axum::{routing::get, Router};
    use axum_extra::extract::cookie::Key;
    use db::{error::DbError, mock::MockDb};
    use hyper::{header, Body, Request, StatusCode};
    use mockall::predicate::*;
    use settings::Settings;
    use std::{result, sync::Arc};
    use storage::kellnr_crate_storage::KellnrCrateStorage;
    use tower::ServiceExt;

    async fn admin_endpoint(user: MaybeUser) -> result::Result<(), RouteError> {
        user.assert_admin()?;
        Ok(())
    }

    async fn normal_endpoint(user: MaybeUser) -> result::Result<(), RouteError> {
        user.assert_normal()?;
        Ok(())
    }

    async fn any_endpoint(_user: MaybeUser) {}

    async fn app(db: Arc<dyn DbProvider>) -> Router {
        let settings = Settings::new().unwrap();
        Router::new()
            .route("/admin", get(admin_endpoint))
            .route("/normal", get(normal_endpoint))
            .route("/any", get(any_endpoint))
            .with_state(AppStateData {
                db,
                signing_key: Key::from(crate::test_helper::TEST_KEY),
                crate_storage: Arc::new(KellnrCrateStorage::new(&settings).await.unwrap()),
                settings: Arc::new(settings),
                ..appstate::test_state().await
            })
    }

    // AdminUser tests

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    fn c1234() -> String {
        encode_cookies([(constants::COOKIE_SESSION_ID, "1234")])
    }

    #[tokio::test]
    async fn admin_auth_works() -> Result {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("admin".to_string(), true)));

        let r = app(Arc::new(mock_db))
            .await
            .oneshot(
                Request::get("/admin")
                    .header(
                        header::COOKIE,
                        encode_cookies([(constants::COOKIE_SESSION_ID, "1234")]),
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
            .await
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
            .await
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
            .await
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
            .await
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
            .await
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
            .await
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
            .await
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
            .await
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
            .await
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
            .await
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
            .await
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
