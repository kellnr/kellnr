use crate::error::RouteError;
use axum::http::request::Parts;
use axum::RequestPartsExt;
use axum_extra::extract::PrivateCookieJar;
use db::DbProvider;
use rocket::http::{Cookie, Status};
use rocket::outcome::Outcome::*;
use rocket::request::{self, FromRequest, Request};
use rocket::State;
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

// TODO(ItsEthra): A better idea would probably to use DbError and use other variants for different purposes
#[derive(Debug)]
pub enum LoginError {
    Invalid(String),
    NoSettings(String),
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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AnyUser {
    type Error = LoginError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        #[allow(clippy::borrowed_box)]
        async fn validate(
            db: &Box<dyn DbProvider>,
            cookie: &Cookie<'_>,
        ) -> request::Outcome<AnyUser, LoginError> {
            match db.validate_session(cookie.value()).await {
                Ok((name, _)) => Success(Self(name)),
                Err(e) => Failure((Status::Unauthorized, LoginError::Invalid(e.to_string()))),
            }
        }

        let db = match get_db(request).await {
            Ok(s) => s,
            Err(e) => return Failure(e),
        };

        let session_cookie = request.cookies().get_private(constants::COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => validate(db, &cookie).await,
            None => Forward(()),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for NormalUser {
    type Error = LoginError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        #[allow(clippy::borrowed_box)]
        async fn validate(
            db: &Box<dyn DbProvider>,
            cookie: &Cookie<'_>,
        ) -> request::Outcome<NormalUser, LoginError> {
            match db.validate_session(cookie.value()).await {
                Ok((name, false)) => Success(Self(name)),
                Ok((_, true)) => Forward(()),
                Err(e) => Failure((Status::Unauthorized, LoginError::Invalid(e.to_string()))),
            }
        }

        let db = match get_db(request).await {
            Ok(s) => s,
            Err(e) => return Failure(e),
        };

        let session_cookie = request.cookies().get_private(constants::COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => validate(db, &cookie).await,
            None => Forward(()),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminUser {
    type Error = LoginError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        #[allow(clippy::borrowed_box)]
        async fn validate(
            db: &Box<dyn DbProvider>,
            cookie: &Cookie<'_>,
        ) -> request::Outcome<AdminUser, LoginError> {
            match db.validate_session(cookie.value()).await {
                Ok((name, true)) => Success(Self(name)),
                Ok((_, false)) => Forward(()),
                Err(e) => Failure((Status::Unauthorized, LoginError::Invalid(e.to_string()))),
            }
        }

        let db = match get_db(request).await {
            Ok(s) => s,
            Err(e) => return Failure(e),
        };

        let session_cookie = request.cookies().get_private(constants::COOKIE_SESSION_ID);
        match session_cookie {
            Some(cookie) => validate(db, &cookie).await,
            None => Forward(()),
        }
    }
}

async fn get_db<'r>(
    request: &'r Request<'_>,
) -> Result<&'r State<Box<dyn DbProvider>>, (Status, LoginError)> {
    let db = request.guard::<&State<Box<dyn DbProvider>>>().await;
    match db {
        Success(s) => Ok(s),
        Failure(e) => Err((
            Status::InternalServerError,
            LoginError::NoSettings(e.0.to_string()),
        )),
        Forward(_) => Err((
            Status::InternalServerError,
            LoginError::NoSettings("Forward instead of getting db.".to_string()),
        )),
    }
}

#[cfg(test)]
mod session_tests {
    use std::{borrow::Cow, result, sync::Arc};

    use super::*;
    use appstate::AppStateData;
    use axum::{routing::get, Router};
    use axum_extra::extract::cookie::Key;
    use cookie::CookieJar;
    use db::{error::DbError, mock::MockDb};
    use hyper::{header, Body, Request, StatusCode};
    use mockall::predicate::*;
    use settings::Settings;
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

    const TEST_KEY: &[u8] = &[1; 64];

    fn app(db: Arc<dyn DbProvider>) -> Router {
        Router::new()
            .route("/admin", get(admin_endpoint))
            .route("/normal", get(normal_endpoint))
            .route("/any", get(any_endpoint))
            .with_state(AppStateData {
                db,
                signing_key: Key::from(TEST_KEY),
                // TODO(ItsEthra): impl Default for Settings
                settings: Arc::new(Settings::new().unwrap()),
            })
    }

    // AdminUser tests

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    // there has to be a better way to set cookies, i really don't like improrting cookie crate just to do this
    fn encode_cookies<const N: usize, K: Into<Cow<'static, str>>, V: Into<Cow<'static, str>>>(
        cookies: [(K, V); N],
    ) -> String {
        let mut clear = CookieJar::new();
        let mut jar = clear.private_mut(&TEST_KEY.try_into().unwrap());
        cookies
            .into_iter()
            .for_each(|(k, v)| jar.add(Cookie::new(k, v)));
        clear
            .iter()
            .map(|c| c.encoded().to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }

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
