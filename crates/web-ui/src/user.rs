use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::Cookie;
use cookie::time;
use kellnr_appstate::{AppState, DbState};
use kellnr_auth::token;
use kellnr_common::util::generate_rand_string;
use kellnr_db::password::generate_salt;
use kellnr_db::{self, AuthToken, User};
use kellnr_settings::constants::{COOKIE_SESSION_ID, COOKIE_SESSION_USER};
use serde::{Deserialize, Serialize};

use crate::error::RouteError;
use crate::session::MaybeUser;

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

#[derive(Deserialize)]
pub struct ReadOnlyState {
    pub state: bool,
}

pub async fn read_only(
    user: MaybeUser,
    Path(name): Path<String>,
    State(db): DbState,
    Json(ro_state): Json<ReadOnlyState>,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    Ok(db.change_read_only_state(&name, ro_state.state).await?)
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

impl Credentials {
    pub fn validate(&self) -> Result<(), RouteError> {
        if self.user.is_empty() {
            return Err(RouteError::Status(StatusCode::BAD_REQUEST));
        }
        if self.pwd.is_empty() {
            return Err(RouteError::Status(StatusCode::BAD_REQUEST));
        }
        Ok(())
    }
}

pub async fn login(
    cookies: PrivateCookieJar,
    State(state): AppState,
    Json(credentials): Json<Credentials>,
) -> Result<(PrivateCookieJar, Json<LoggedInUser>), RouteError> {
    credentials.validate()?;

    let user = state
        .db
        .authenticate_user(&credentials.user, &credentials.pwd)
        .await
        .map_err(|_| RouteError::AuthenticationFailure)?;

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

#[expect(clippy::unused_async)] // part of the router
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
            user: String::new(),
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

impl PwdChange {
    pub fn validate(&self) -> Result<(), RouteError> {
        if self.old_pwd.is_empty() {
            return Err(RouteError::Status(StatusCode::BAD_REQUEST));
        }
        if self.new_pwd1.is_empty() || self.new_pwd2.is_empty() {
            return Err(RouteError::Status(StatusCode::BAD_REQUEST));
        }
        if self.new_pwd1 != self.new_pwd2 {
            return Err(RouteError::Status(StatusCode::BAD_REQUEST));
        }
        Ok(())
    }
}

pub async fn change_pwd(
    user: MaybeUser,
    State(db): DbState,
    Json(pwd_change): Json<PwdChange>,
) -> Result<(), RouteError> {
    pwd_change.validate()?;

    let Ok(user) = db.authenticate_user(user.name(), &pwd_change.old_pwd).await else {
        return Err(RouteError::Status(StatusCode::BAD_REQUEST));
    };

    db.change_pwd(&user.name, &pwd_change.new_pwd1).await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct NewUser {
    pub pwd1: String,
    pub pwd2: String,
    pub name: String,
    #[serde(default)] // Set to false if not in message from client
    pub is_admin: bool,
    #[serde(default)] // Set to false if not in message from client
    pub is_read_only: bool,
}

impl NewUser {
    pub fn validate(&self) -> Result<(), RouteError> {
        if self.name.is_empty() {
            return Err(RouteError::Status(StatusCode::BAD_REQUEST));
        }
        if self.pwd1.is_empty() || self.pwd2.is_empty() {
            return Err(RouteError::Status(StatusCode::BAD_REQUEST));
        }
        if self.pwd1 != self.pwd2 {
            return Err(RouteError::Status(StatusCode::BAD_REQUEST));
        }
        Ok(())
    }
}

pub async fn add(
    user: MaybeUser,
    State(db): DbState,
    Json(new_user): Json<NewUser>,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    new_user.validate()?;

    let salt = generate_salt();
    Ok(db
        .add_user(
            &new_user.name,
            &new_user.pwd1,
            &salt,
            new_user.is_admin,
            new_user.is_read_only,
        )
        .await?)
}
