use crate::error::RouteError;
use axum::http::request::Parts;
use axum::response::IntoResponse;
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

impl IntoResponse for LoginError {
    fn into_response(self) -> axum::response::Response {
        match self {
            LoginError::Invalid(e) => (axum::http::StatusCode::BAD_REQUEST, e).into_response(),
            LoginError::NoSettings(e) => (axum::http::StatusCode::BAD_REQUEST, e).into_response(),
        }
    }
}

pub enum MaybeUser {
    Guest,
    // Consider using a db model or something?
    Normal(String),
    Admin(String),
}

impl MaybeUser {
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Guest => None,
            Self::Normal(name) | Self::Admin(name) => Some(name),
        }
    }

    pub fn assert_atleast_normal(&self) -> Result<(), RouteError> {
        match self {
            MaybeUser::Normal(_) | MaybeUser::Admin(_) => Ok(()),
            _ => Err(RouteError::Status(axum::http::StatusCode::FORBIDDEN)),
        }
    }

    pub fn assert_admin(&self) -> Result<(), RouteError> {
        match self {
            MaybeUser::Admin(_) => Ok(()),
            _ => Err(RouteError::Status(axum::http::StatusCode::FORBIDDEN)),
        }
    }
}

#[axum::async_trait]
impl axum::extract::FromRequestParts<appstate::AppStateData> for MaybeUser {
    type Rejection = LoginError;

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
                Err(e) => Err(LoginError::Invalid(e.to_string())),
            },
            None => Ok(Self::Guest),
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
    use super::*;
    use db::error::DbError;
    use db::mock::MockDb;
    use mockall::predicate::*;
    use rocket::local::blocking::Client;
    use rocket::{get, routes};

    #[get("/admin")]
    fn admin_endpoint(user: AdminUser) {
        assert_eq!("admin", user.name());
    }

    #[get("/normal")]
    fn normal_endpoint(user: NormalUser) {
        assert_eq!("normal", user.name());
    }

    #[get("/any")]
    fn any_endpoint(user: AnyUser) {
        assert_eq!("any", user.name());
    }

    fn rocket_conf() -> rocket::config::Config {
        use rocket::config::{Config, SecretKey};
        Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        }
    }

    // AdminUser tests

    #[test]
    fn admin_auth_works() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("admin".to_string(), true)));

        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![admin_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client
            .get("/admin")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "1234"));

        let result = req.dispatch();

        assert_eq!(result.status(), Status::Ok);
    }

    #[test]
    fn admin_auth_user_is_no_admin() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("admin".to_string(), false)));

        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![admin_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client
            .get("/admin")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "1234"));

        let result = req.dispatch();

        assert_eq!(result.status(), Status::NotFound);
    }

    #[test]
    fn admin_auth_user_but_no_cookie_sent() {
        let mock_db = MockDb::new();
        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![admin_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client.get("/admin");

        let result = req.dispatch();

        // NotFound as the forward isn't caught by any route
        assert_eq!(result.status(), Status::NotFound);
    }

    #[test]
    fn admin_auth_user_but_no_cookie_in_store() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Err(DbError::SessionNotFound));

        use rocket::config::{Config, SecretKey};
        let rocket_conf = Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        };

        let rocket = rocket::custom(rocket_conf)
            .mount("/", routes![admin_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client
            .get("/admin")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "1234"));

        let result = req.dispatch();

        assert_eq!(result.status(), Status::Unauthorized);
    }

    // NormalUser tests

    #[test]
    fn normal_auth_works() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("normal".to_string(), false)));

        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![normal_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client
            .get("/normal")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "1234"));

        let result = req.dispatch();

        assert_eq!(result.status(), Status::Ok);
    }

    #[test]
    fn normal_auth_user_is_admin() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("normal".to_string(), true)));

        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![normal_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client
            .get("/normal")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "1234"));

        let result = req.dispatch();

        assert_eq!(result.status(), Status::NotFound);
    }

    #[test]
    fn normal_auth_user_but_no_cookie_sent() {
        let mock_db = MockDb::new();
        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![normal_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client.get("/normal");

        let result = req.dispatch();

        // NotFound as the forward isn't caught by any route
        assert_eq!(result.status(), Status::NotFound);
    }

    #[test]
    fn normal_auth_user_but_no_cookie_in_store() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Err(DbError::SessionNotFound));

        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![normal_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client
            .get("/normal")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "1234"));

        let result = req.dispatch();

        assert_eq!(result.status(), Status::Unauthorized);
    }

    // AnyUser tests

    #[test]
    fn any_auth_user_is_normal() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("any".to_string(), false)));

        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![any_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client
            .get("/any")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "1234"));

        let result = req.dispatch();

        assert_eq!(result.status(), Status::Ok);
    }

    #[test]
    fn any_auth_user_is_admin() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Ok(("any".to_string(), true)));

        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![any_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client
            .get("/any")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "1234"));

        let result = req.dispatch();

        assert_eq!(result.status(), Status::Ok);
    }

    #[test]
    fn any_auth_user_but_no_cookie_sent() {
        let mock_db = MockDb::new();
        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![any_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client.get("/any");

        let result = req.dispatch();

        // NotFound as the forward isn't caught by any route
        assert_eq!(result.status(), Status::NotFound);
    }

    #[test]
    fn any_auth_user_but_no_cookie_in_store() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("1234"))
            .returning(|_st| Err(DbError::SessionNotFound));

        let rocket = rocket::custom(rocket_conf())
            .mount("/", routes![any_endpoint])
            .manage(Box::new(mock_db) as Box<dyn DbProvider>);

        let client = Client::tracked(rocket).expect("valid rocket client");
        let req = client
            .get("/any")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "1234"));

        let result = req.dispatch();

        assert_eq!(result.status(), Status::Unauthorized);
    }
}
