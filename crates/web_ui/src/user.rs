use crate::error::RouteError;
use crate::session::MaybeUser;
use appstate::{AppState, DbState};
use auth::token;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::PrivateCookieJar;
use common::util::generate_rand_string;
use cookie::time;
use db::password::generate_salt;
use db::{self, AuthToken, User};
use serde::{Deserialize, Serialize};
use settings::constants::*;

#[derive(Serialize)]
pub struct NewTokenResponse {
    name: String,
    token: String,
}

pub async fn add_token(
    user: MaybeUser,
    State(db): DbState,
    Json(auth_token): Json<token::NewTokenReqData>,
) -> Result<Json<NewTokenResponse>, RouteError> {
    let token = token::generate_token();
    db.add_auth_token(&auth_token.name, &token, user.name())
        .await?;

    Ok(NewTokenResponse {
        name: auth_token.name.clone(),
        token,
    }
    .into())
}

pub async fn list_tokens(
    user: MaybeUser,
    State(db): DbState,
) -> Result<Json<Vec<AuthToken>>, RouteError> {
    Ok(Json(db.get_auth_tokens(user.name()).await?))
}

pub async fn list_users(
    user: MaybeUser,
    State(db): DbState,
) -> Result<Json<Vec<User>>, RouteError> {
    user.assert_admin()?;

    Ok(Json(db.get_users().await?))
}

pub async fn delete_token(
    user: MaybeUser,
    Path(id): Path<i32>,
    State(db): DbState,
) -> Result<(), RouteError> {
    db.get_auth_tokens(user.name())
        .await?
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| RouteError::Status(StatusCode::BAD_REQUEST))?;

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
        user: user.name().to_owned(),
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
        Cookie::build((COOKIE_SESSION_ID, session_token))
            .max_age(time::Duration::seconds(
                state.settings.registry.session_age_seconds as i64,
            ))
            .same_site(axum_extra::extract::cookie::SameSite::Strict)
            .path("/"),
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

pub async fn login_state(user: Option<MaybeUser>) -> Json<LoggedInUser> {
    match user {
        Some(MaybeUser::Normal(user)) => LoggedInUser {
            user,
            is_admin: false,
            is_logged_in: true,
        },
        Some(MaybeUser::Admin(user)) => LoggedInUser {
            user,
            is_admin: true,
            is_logged_in: true,
        },
        None => LoggedInUser {
            user: "".to_owned(),
            is_admin: false,
            is_logged_in: false,
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

    jar = jar.remove(COOKIE_SESSION_ID);
    jar = jar.remove(Cookie::build((COOKIE_SESSION_USER, "")).path("/"));

    state.db.delete_session_token(&session_id).await?;
    Ok(jar)
}

#[derive(Deserialize)]
pub struct PwdChange {
    pub old_pwd: String,
    pub new_pwd1: String,
    pub new_pwd2: String,
}

pub async fn change_pwd(
    user: MaybeUser,
    State(db): DbState,
    Json(pwd_change): Json<PwdChange>,
) -> Result<(), RouteError> {
    let Ok(user) = db.authenticate_user(user.name(), &pwd_change.old_pwd).await else {
        return Err(RouteError::Status(StatusCode::BAD_REQUEST));
    };

    if pwd_change.new_pwd1 != pwd_change.new_pwd2 {
        return Err(RouteError::Status(StatusCode::BAD_REQUEST));
    }

    db.change_pwd(&user.name, &pwd_change.new_pwd1).await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct NewUser {
    pub pwd1: String,
    pub pwd2: String,
    pub name: String,
    #[serde(default)] // Set to false of not in message from client
    pub is_admin: bool,
}

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
