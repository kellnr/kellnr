use crate::token::Token;
use error::error::ApiError;
use rocket::http::Status;
use rocket::outcome::Outcome::*;
use rocket::request::{self, FromRequest, Request};
use rocket::State;
use settings::Settings;

// This token checks if "auth_required = true" and if so, it requires a token.
// Else, it does not require a token.
// Returns None if "auth_required = false", else returns Some(Token) or an error.
// Feature is only available in Enterprise version.
#[derive(Debug)]
pub struct AuthReqToken(Option<Token>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthReqToken {
    type Error = ApiError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let settings = match get_settings(request).await {
            Ok(s) => s,
            Err(e) => return Failure(e),
        };

        if settings.auth_required {
            Token::from_request(request).await.map(|t| Self(Some(t)))
        } else {
            Success(Self(None))
        }
    }
}

async fn get_settings<'r>(
    request: &'r Request<'_>,
) -> Result<&'r State<Settings>, (Status, ApiError)> {
    match request.guard::<&State<Settings>>().await {
        Success(s) => Ok(s),
        Failure(e) => Err((Status::InternalServerError, ApiError::from(&e.0))),
        Forward(_) => Err((
            Status::InternalServerError,
            ApiError::from("Forward instead of getting settings"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::error::DbError;
    use db::mock::MockDb;
    use db::{DbProvider, User};
    use mockall::predicate::*;
    use rocket::config::{Config, SecretKey};
    use rocket::get;
    use rocket::http::{Header, Status};
    use rocket::local::blocking::Client;
    use rocket::routes;
    use settings::Settings;

    #[test]
    fn no_auth_required() {
        let settings = Settings {
            auth_required: false,
            ..Settings::new().unwrap()
        };
        let client = client(settings);
        let req = client.get("/api/v1/test");

        let response = req.dispatch();

        assert_eq!(response.status(), Status::Ok);
    }

    #[test]
    fn auth_required_but_not_provided() {
        let settings = Settings {
            auth_required: true,
            ..Settings::new().unwrap()
        };
        let client = client(settings);
        let req = client.get("/api/v1/test");

        let response = req.dispatch();

        assert_eq!(response.status(), Status::Unauthorized);
    }

    #[test]
    fn auth_required_but_wrong_token_provided() {
        let settings = Settings {
            auth_required: true,
            ..Settings::new().unwrap()
        };
        let client = client(settings);
        let req = client
            .get("/api/v1/test")
            .header(Header::new("Authorization", "wrong_token"));

        let response = req.dispatch();

        assert_eq!(response.status(), Status::Forbidden);
    }

    #[test]
    fn auth_required_and_right_token_provided() {
        let settings = Settings {
            auth_required: true,
            ..Settings::new().unwrap()
        };
        let client = client(settings);
        let req = client
            .get("/api/v1/test")
            .header(Header::new("Authorization", "token"));

        let response = req.dispatch();

        assert_eq!(response.status(), Status::Ok);
    }

    #[get("/test")]
    pub async fn test_auth_req_token(auth_req_token: AuthReqToken) {
        _ = auth_req_token;
    }

    fn client(settings: Settings) -> Client {
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

        let db = Box::new(mock_db) as Box<dyn DbProvider>;

        let rocket_conf = Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        };

        let rocket = rocket::custom(rocket_conf)
            .mount("/api/v1/", routes![test_auth_req_token])
            .manage(db)
            .manage(settings);

        Client::tracked(rocket).unwrap()
    }
}
