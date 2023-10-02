use crate::session::{AdminUser, AnyUser, Name};
use auth::token;
use axum::http::StatusCode;
use common::util::generate_rand_string;
use db::password::generate_salt;
use db::DbProvider;
use db::{self, AuthToken, User};
use rocket::http::{Cookie, CookieJar, SameSite, Status};
use rocket::response::status;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::{get, post, State};
use settings::constants::*;
use settings::Settings;
use std::sync::Arc;

#[derive(Serialize)]
pub struct NewTokenResponse {
    name: String,
    token: String,
}

#[post("/add_token", data = "<auth_token>")]
pub async fn add_token(
    user: AnyUser,
    auth_token: Json<token::NewTokenReqData>,
    db: &State<Box<dyn DbProvider>>,
) -> Result<Json<NewTokenResponse>, status::Custom<&'static str>> {
    let token = token::generate_token();
    match db
        .add_auth_token(&auth_token.name, &token, &user.name())
        .await
    {
        Err(_) => Err(status::Custom(
            Status::InternalServerError,
            "Unable to add authentication token to database.",
        )),
        Ok(_) => Ok(Json(NewTokenResponse {
            name: auth_token.name.clone(),
            token,
        })),
    }
}

#[derive(Serialize)]
pub struct AuthTokenList {
    tokens: Vec<AuthToken>,
}

#[get("/list_tokens")]
pub async fn list_tokens(
    user: AnyUser,
    db: &State<Box<dyn DbProvider>>,
) -> Result<Json<AuthTokenList>, status::Custom<&'static str>> {
    match db.get_auth_tokens(&user.name()).await {
        Ok(tokens) => Ok(Json(AuthTokenList { tokens })),
        Err(_) => Err(status::Custom(
            Status::InternalServerError,
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

#[get("/delete_token/<id>")]
pub async fn delete_token(user: AnyUser, id: i32, db: &State<Box<dyn DbProvider>>) -> Status {
    let _ = user;
    match db.delete_auth_token(id).await {
        Ok(_) => Status::Ok,
        Err(_) => Status::InternalServerError,
    }
}

#[derive(Serialize)]
pub struct ResetPwd {
    new_pwd: String,
    user: String,
}

#[get("/resetpwd/<name>")]
pub async fn reset_pwd(
    user: AdminUser,
    name: String,
    db: &State<Box<dyn DbProvider>>,
) -> Result<Json<ResetPwd>, status::Custom<&'static str>> {
    let new_pwd = generate_rand_string(12);
    match db.change_pwd(&name, &new_pwd).await {
        Err(_) => Err(status::Custom(
            Status::InternalServerError,
            "Unable to reset user password.",
        )),
        Ok(_) => Ok(Json(ResetPwd {
            new_pwd,
            user: user.name(),
        })),
    }
}

#[get("/delete/<name>")]
pub async fn delete(user: AdminUser, name: String, db: &State<Box<dyn DbProvider>>) -> Status {
    let _ = user;
    match db.delete_user(&name).await {
        Ok(_) => Status::Ok,
        Err(_) => Status::InternalServerError,
    }
}

#[get("/delete/<name>", rank = 2)]
pub fn delete_forbidden(name: String) -> Status {
    // If a user without admin rights tries to delete an user, throw a 403
    let _ = name;
    Status::Forbidden
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
    axum::extract::State(state): appstate::AppState,
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

#[get("/logout")]
pub async fn logout(cookies: &CookieJar<'_>, db: &State<Box<dyn DbProvider>>) -> Status {
    let session_id = match cookies.get_private(COOKIE_SESSION_ID) {
        Some(c) => c.value().to_owned(),
        None => return Status::Ok, // Already logged out as no cookie can be found
    };

    cookies.remove_private(Cookie::named(COOKIE_SESSION_ID));
    cookies.remove(Cookie::build(COOKIE_SESSION_USER, "").path("/").finish());

    if db.delete_session_token(&session_id).await.is_err() {
        Status::InternalServerError
    } else {
        Status::Ok
    }
}

#[derive(Deserialize)]
pub struct PwdChange {
    pub old_pwd: String,
    pub new_pwd1: String,
    pub new_pwd2: String,
}

#[post("/changepwd", data = "<pwd_change>")]
pub async fn change_pwd(
    user: AnyUser,
    pwd_change: Json<PwdChange>,
    db: &State<Box<dyn DbProvider>>,
) -> Status {
    let user = match db
        .authenticate_user(&user.name(), &pwd_change.old_pwd)
        .await
    {
        Ok(user) => user,
        Err(_) => return Status::BadRequest,
    };

    if pwd_change.new_pwd1 != pwd_change.new_pwd2 {
        return Status::BadRequest;
    }

    match db.change_pwd(&user.name, &pwd_change.new_pwd1).await {
        Ok(_) => Status::Ok,
        Err(_) => Status::InternalServerError,
    }
}

#[derive(Deserialize)]
pub struct NewUser {
    pub pwd1: String,
    pub pwd2: String,
    pub name: String,
    #[serde(default)] // Set to false of not in message from client
    pub is_admin: bool,
}

#[post("/add", data = "<new_user>")]
pub async fn add(
    user: AdminUser,
    new_user: Json<NewUser>,
    db: &State<Box<dyn DbProvider>>,
) -> Status {
    let _ = user;

    if new_user.pwd1 != new_user.pwd2 {
        return Status::BadRequest;
    }

    let salt = generate_salt();
    match db
        .add_user(&new_user.name, &new_user.pwd1, &salt, new_user.is_admin)
        .await
    {
        Ok(_) => Status::Ok,
        Err(_) => Status::InternalServerError,
    }
}
