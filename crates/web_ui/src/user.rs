use crate::session::{AdminUser, AnyUser, MaybeUser, Name};
use appstate::AppState;
use auth::token;
use axum::extract::Path;
use axum::http::StatusCode;
use common::util::generate_rand_string;
use db::password::generate_salt;
use db::DbProvider;
use db::{self, AuthToken, User};
use rocket::http::{Cookie, SameSite, Status};
use rocket::response::status;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::{get, State};
use settings::constants::*;

#[derive(Serialize)]
pub struct NewTokenResponse {
    name: String,
    token: String,
}

// #[post("/add_token", data = "<auth_token>")]
pub async fn add_token(
    user: MaybeUser,
    axum::extract::State(state): appstate::AppState,
    axum::Json(auth_token): axum::Json<token::NewTokenReqData>,
) -> Result<axum::Json<NewTokenResponse>, (StatusCode, &'static str)> {
    user.assert_atleast_normal()
        .map_err(|c| (c, "Guests are forbidden"))?;

    let token = token::generate_token();
    match state
        .db
        .add_auth_token(&auth_token.name, &token, user.name().unwrap())
        .await
    {
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to add authentication token to database.",
        )),
        Ok(_) => Ok(NewTokenResponse {
            name: auth_token.name.clone(),
            token,
        }
        .into()),
    }
}

#[derive(Serialize)]
pub struct AuthTokenList {
    tokens: Vec<AuthToken>,
}

pub async fn list_tokens(
    user: MaybeUser,
    axum::extract::State(state): appstate::AppState,
) -> Result<axum::Json<AuthTokenList>, (StatusCode, &'static str)> {
    user.assert_atleast_normal()
        .map_err(|c| (c, "Guests are forbidden"))?;

    match state.db.get_auth_tokens(user.name().unwrap()).await {
        Ok(tokens) => Ok(AuthTokenList { tokens }.into()),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to fetch authentication tokens from database.",
        )),
    }
}

#[derive(Serialize)]
pub struct UserList {
    users: Vec<User>,
}

#[get("/list_users")]
pub async fn list_users(
    user: AdminUser,
    db: &State<Box<dyn DbProvider>>,
) -> Result<Json<UserList>, status::Custom<&'static str>> {
    let _ = user;
    match db.get_users().await {
        Ok(users) => Ok(Json(UserList { users })),
        Err(_) => Err(status::Custom(
            Status::InternalServerError,
            "Unable to fetch users from database.",
        )),
    }
}

pub async fn delete_token(
    user: MaybeUser,
    Path(id): Path<i32>,
    axum::extract::State(state): appstate::AppState,
) -> Result<(), axum::http::StatusCode> {
    user.assert_atleast_normal()?;

    // TODO(ItsEthra): Should be checking if user owns the token i believe

    match state.db.delete_auth_token(id).await {
        Ok(_) => Ok(()),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Serialize)]
pub struct ResetPwd {
    new_pwd: String,
    user: String,
}

pub async fn reset_pwd(
    user: MaybeUser,
    Path(name): Path<String>,
    axum::extract::State(state): appstate::AppState,
) -> Result<axum::Json<ResetPwd>, (StatusCode, &'static str)> {
    user.assert_admin()
        .map_err(|c| (c, "Insufficient privileges"))?;

    let new_pwd = generate_rand_string(12);
    match state.db.change_pwd(&name, &new_pwd).await {
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to reset user password.",
        )),
        Ok(_) => Ok(ResetPwd {
            // TODO(ItsEthra): I wasn't looking at frontend code at all, was using admin's name intended,
            // or should it be target user's name?
            user: user.name().unwrap().to_owned(),
            new_pwd,
        }
        .into()),
    }
}

pub async fn delete(
    user: MaybeUser,
    Path(name): Path<String>,
    axum::extract::State(state): appstate::AppState,
) -> Result<(), StatusCode> {
    user.assert_admin()?;

    match state.db.delete_user(&name).await {
        Ok(_) => Ok(()),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Serialize)]
pub struct LoggedInUser {
    user: String,
    is_admin: bool,
    is_logged_in: bool,
}

#[derive(Deserialize)]
pub struct Credentials {
    pub user: String,
    pub pwd: String,
}

pub async fn login(
    cookies: axum_extra::extract::CookieJar,
    axum::extract::State(state): AppState,
    axum::extract::Json(credentials): axum::extract::Json<Credentials>,
) -> Result<
    (
        axum_extra::extract::CookieJar,
        axum::extract::Json<LoggedInUser>,
    ),
    StatusCode,
> {
    match state
        .db
        .authenticate_user(&credentials.user, &credentials.pwd)
        .await
    {
        Ok(user) => {
            let session_token = generate_rand_string(12);
            if state
                .db
                .add_session_token(&credentials.user, &session_token)
                .await
                .is_err()
            {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }

            let jar = cookies.add(
                Cookie::build(COOKIE_SESSION_ID, session_token)
                    .max_age(rocket::time::Duration::seconds(
                        state.settings.session_age_seconds as i64,
                    ))
                    .same_site(SameSite::Strict)
                    .finish(),
            );

            Ok((
                jar,
                LoggedInUser {
                    user: credentials.user.clone(),
                    is_admin: user.is_admin,
                    is_logged_in: true,
                }
                .into(),
            ))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[get("/login_state")]
pub async fn login_state(
    user: Option<AnyUser>,
    db: &State<Box<dyn DbProvider>>,
) -> Result<Json<LoggedInUser>, Status> {
    match user {
        Some(user) => match db.get_user(&user.name()).await {
            Ok(user) => Ok(Json(LoggedInUser {
                user: user.name,
                is_admin: user.is_admin,
                is_logged_in: true,
            })),
            Err(_) => Err(Status::InternalServerError),
        },
        None => Ok(Json(LoggedInUser {
            user: String::new(),
            is_admin: false,
            is_logged_in: false,
        })),
    }
}

pub async fn logout(
    mut jar: axum_extra::extract::CookieJar,
    axum::extract::State(state): AppState,
) -> (axum_extra::extract::CookieJar, axum::http::StatusCode) {
    let session_id = match jar.get(COOKIE_SESSION_ID) {
        Some(c) => c.value().to_owned(),
        None => return (jar, StatusCode::OK), // Already logged out as no cookie can be found
    };

    jar = jar.remove(Cookie::named(COOKIE_SESSION_ID));
    jar = jar.remove(Cookie::build(COOKIE_SESSION_USER, "").path("/").finish());

    let code = if state.db.delete_session_token(&session_id).await.is_err() {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    };

    (jar, code)
}

#[derive(Deserialize)]
pub struct PwdChange {
    pub old_pwd: String,
    // Maybe checking on client is enough?
    pub new_pwd1: String,
    pub new_pwd2: String,
}

pub async fn change_pwd(
    user: MaybeUser,
    axum::extract::State(state): appstate::AppState,
    axum::extract::Json(pwd_change): axum::extract::Json<PwdChange>,
) -> StatusCode {
    let Some(name) = user.name() else {
        return StatusCode::UNAUTHORIZED;
    };

    let Ok(user) = state.db.authenticate_user(name, &pwd_change.old_pwd).await else {
        return StatusCode::BAD_REQUEST;
    };

    if pwd_change.new_pwd1 != pwd_change.new_pwd2 {
        return StatusCode::BAD_REQUEST;
    }

    match state.db.change_pwd(&user.name, &pwd_change.new_pwd1).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Deserialize)]
pub struct NewUser {
    // TODO(ItsEthra): Consider checking only on client
    pub pwd1: String,
    pub pwd2: String,
    pub name: String,
    #[serde(default)] // Set to false of not in message from client
    pub is_admin: bool,
}

// #[post("/add", data = "<new_user>")]
pub async fn add(
    user: MaybeUser,
    axum::extract::State(state): appstate::AppState,
    axum::extract::Json(new_user): axum::extract::Json<NewUser>,
) -> Result<(), StatusCode> {
    user.assert_admin()?;

    if new_user.pwd1 != new_user.pwd2 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let salt = generate_salt();
    match state
        .db
        .add_user(&new_user.name, &new_user.pwd1, &salt, new_user.is_admin)
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
