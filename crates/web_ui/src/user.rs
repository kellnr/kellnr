use crate::session::MaybeUser;
use appstate::{AppState, DbState};
use auth::token;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::PrivateCookieJar;
use common::util::generate_rand_string;
use db::password::generate_salt;
use db::{self, AuthToken, User};
use serde::{Deserialize, Serialize};
use settings::constants::*;

#[derive(Serialize)]
pub struct NewTokenResponse {
    name: String,
    token: String,
}

pub enum RouteError {
    DbError(db::error::DbError),
    InsufficientPrivileges,
    Status(StatusCode),
}

impl From<db::error::DbError> for RouteError {
    fn from(err: db::error::DbError) -> Self {
        Self::DbError(err)
    }
}

impl IntoResponse for RouteError {
    fn into_response(self) -> Response {
        match self {
            RouteError::DbError(err) => {
                tracing::error!("Db: {err:?}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            RouteError::Status(status) => status.into_response(),
            RouteError::InsufficientPrivileges => StatusCode::FORBIDDEN.into_response(),
        }
    }
}

// #[post("/add_token", data = "<auth_token>")]
pub async fn add_token(
    user: MaybeUser,
    State(db): DbState,
    Json(auth_token): Json<token::NewTokenReqData>,
) -> Result<Json<NewTokenResponse>, RouteError> {
    user.assert_atleast_normal()?;

    let token = token::generate_token();
    db.add_auth_token(&auth_token.name, &token, user.name().unwrap())
        .await?;

    Ok(NewTokenResponse {
        name: auth_token.name.clone(),
        token,
    }
    .into())
}

// TODO(ItsEthra): This should probably be inlined, i.e just return Json<Vec<AuthToken>> in list_tokens, but for now I'm not touching frontend
#[derive(Serialize)]
pub struct AuthTokenList {
    tokens: Vec<AuthToken>,
}

pub async fn list_tokens(
    user: MaybeUser,
    State(db): DbState,
) -> Result<Json<AuthTokenList>, RouteError> {
    user.assert_atleast_normal()?;

    Ok(db
        .get_auth_tokens(user.name().unwrap())
        .await
        .map(|tokens| AuthTokenList { tokens }.into())?)
}

// TODO(ItsEthra): Same as AuthTokenList
#[derive(Serialize)]
pub struct UserList {
    users: Vec<User>,
}

pub async fn list_users(
    user: MaybeUser,
    State(db): appstate::DbState,
) -> Result<Json<UserList>, RouteError> {
    user.assert_admin()?;

    Ok(db
        .get_users()
        .await
        .map(|users| UserList { users }.into())?)
}

pub async fn delete_token(
    user: MaybeUser,
    Path(id): Path<i32>,
    State(db): DbState,
) -> Result<(), RouteError> {
    user.assert_atleast_normal()?;

    // TODO(ItsEthra): Should be checking if user owns the token i believe

    Ok(db.delete_auth_token(id).await?)
}

#[derive(Serialize)]
pub struct ResetPwd {
    new_pwd: String,
    user: String,
}

pub async fn reset_pwd(
    user: MaybeUser,
    Path(name): Path<String>,
    State(db): DbState,
) -> Result<Json<ResetPwd>, RouteError> {
    user.assert_admin()?;

    let new_pwd = generate_rand_string(12);
    db.change_pwd(&name, &new_pwd).await?;

    Ok(ResetPwd {
        // TODO(ItsEthra): I wasn't looking at frontend code at all, was using admin's name intended,
        // or should it be target user's name?
        user: user.name().unwrap().to_owned(),
        new_pwd,
    }
    .into())
}

pub async fn delete(
    user: MaybeUser,
    Path(name): Path<String>,
    State(db): DbState,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    Ok(db.delete_user(&name).await?)
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
    cookies: PrivateCookieJar,
    State(state): appstate::AppState,
    Json(credentials): Json<Credentials>,
) -> Result<(PrivateCookieJar, Json<LoggedInUser>), RouteError> {
    let user = state
        .db
        .authenticate_user(&credentials.user, &credentials.pwd)
        .await?;

    let session_token = generate_rand_string(12);
    state
        .db
        .add_session_token(&credentials.user, &session_token)
        .await?;

    let jar = cookies.add(
        Cookie::build(COOKIE_SESSION_ID, session_token)
            .max_age(rocket::time::Duration::seconds(
                state.settings.session_age_seconds as i64,
            ))
            .same_site(axum_extra::extract::cookie::SameSite::Strict)
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

pub async fn login_state(user: MaybeUser) -> Json<LoggedInUser> {
    match user {
        MaybeUser::Guest => LoggedInUser {
            user: "".into(),
            is_admin: false,
            is_logged_in: false,
        },
        MaybeUser::Normal(user) => LoggedInUser {
            user,
            is_admin: false,
            is_logged_in: true,
        },
        MaybeUser::Admin(user) => LoggedInUser {
            user,
            is_admin: true,
            is_logged_in: true,
        },
    }
    .into()
}

pub async fn logout(
    mut jar: PrivateCookieJar,
    State(state): AppState,
) -> Result<PrivateCookieJar, RouteError> {
    let session_id = match jar.get(COOKIE_SESSION_ID) {
        Some(c) => c.value().to_owned(),
        None => return Ok(jar), // Already logged out as no cookie can be found
    };

    jar = jar.remove(Cookie::named(COOKIE_SESSION_ID));
    jar = jar.remove(Cookie::build(COOKIE_SESSION_USER, "").path("/").finish());

    state.db.delete_session_token(&session_id).await?;
    Ok(jar)
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
    State(db): DbState,
    Json(pwd_change): Json<PwdChange>,
) -> StatusCode {
    let Some(name) = user.name() else {
        return StatusCode::UNAUTHORIZED;
    };

    let Ok(user) = db.authenticate_user(name, &pwd_change.old_pwd).await else {
        return StatusCode::BAD_REQUEST;
    };

    if pwd_change.new_pwd1 != pwd_change.new_pwd2 {
        return StatusCode::BAD_REQUEST;
    }

    match db.change_pwd(&user.name, &pwd_change.new_pwd1).await {
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
    State(db): DbState,
    Json(new_user): Json<NewUser>,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    if new_user.pwd1 != new_user.pwd2 {
        return Err(RouteError::Status(StatusCode::BAD_REQUEST));
    }

    let salt = generate_salt();
    Ok(db
        .add_user(&new_user.name, &new_user.pwd1, &salt, new_user.is_admin)
        .await?)
}
