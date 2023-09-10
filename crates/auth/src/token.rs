use db::DbProvider;
use error::error::ApiError;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use rocket::http::Status;
use rocket::outcome::Outcome::{self, *};
use rocket::request::{self, FromRequest, Request};
use rocket::serde::Deserialize;
use rocket::State;
use std::iter;

#[derive(Debug)]
pub struct Token {
    pub token: String,
    pub user: String,
    pub is_admin: bool,
}

pub fn generate_token() -> String {
    let mut rng = thread_rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(32)
        .collect::<String>()
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Token {
    type Error = ApiError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let token = request.headers().get_one("Authorization");
        let db = match get_db(request).await {
            Ok(s) => s,
            Err(e) => return Outcome::Failure(e),
        };

        match token {
            Some(token) => {
                let user = match db.get_user_from_token(token).await {
                    Ok(u) => u,
                    Err(_) => {
                        return Outcome::Failure((
                            Status::Forbidden,
                            ApiError::from("Invalid authentication token"),
                        ))
                    }
                };

                Outcome::Success(Token {
                    token: token.to_owned(),
                    user: user.name,
                    is_admin: user.is_admin,
                })
            }
            None => Outcome::Failure((
                Status::Unauthorized,
                ApiError::from("Missing authentication token"),
            )),
        }
    }
}

#[derive(Deserialize)]
pub struct NewTokenReqData {
    pub name: String,
}

async fn get_db<'r>(
    request: &'r Request<'_>,
) -> Result<&'r State<Box<dyn DbProvider>>, (Status, ApiError)> {
    match request.guard::<&State<Box<dyn DbProvider>>>().await {
        Success(s) => Ok(s),
        Failure(e) => Err((Status::InternalServerError, ApiError::from(&e.0))),
        Forward(_) => Err((
            Status::InternalServerError,
            ApiError::from("Forward instead of getting db."),
        )),
    }
}

#[cfg(test)]
mod tests {
    // Indirectly tested through API access to higher level functions
}
