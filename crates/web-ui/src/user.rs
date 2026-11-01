use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::Cookie;
use kellnr_common::account_name::{UserName, validate_token_name};
use kellnr_common::plain_token::PlainToken;
use kellnr_common::util::generate_rand_string;
use kellnr_db::{self, AuthToken, AuthTokenId, DbProvider, User, UserFlags};
use kellnr_http::error::ApiError;
use kellnr_http::identity::{AdminUser, SessionUser};
use kellnr_http::password::{PasswordAuthError, authenticate_password};
use kellnr_http::state::{AppState, DbState, LoginThrottleState, SettingsState, TokenCacheState};
use kellnr_http::token;
use kellnr_settings::Settings;
use kellnr_settings::constants::{COOKIE_OIDC_ID_TOKEN, COOKIE_SESSION_ID, COOKIE_SESSION_USER};
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::ToSchema;

use crate::oauth2::OAuth2Ext;
use crate::session::create_session_jar;

/// Reject local password operations when SSO is enforced.
fn ensure_local_auth_enabled(settings: &Settings) -> Result<(), ApiError> {
    if settings.oauth2.enforced {
        return Err(ApiError::from(StatusCode::FORBIDDEN));
    }
    Ok(())
}

/// Logs `user_name` out everywhere after a password change, except for the
/// session `keep_session_token` if given. API tokens are left untouched so CI
/// pipelines keep working.
async fn revoke_sessions(
    db: &dyn DbProvider,
    user_name: &UserName,
    keep_session_token: Option<&str>,
) -> Result<(), ApiError> {
    match keep_session_token {
        Some(token) => {
            db.delete_other_user_sessions(user_name, &PlainToken::from(token))
                .await?
        }
        None => db.delete_user_sessions(user_name).await?,
    };
    Ok(())
}

#[derive(Serialize, ToSchema)]
pub struct NewTokenResponse {
    name: String,
    token: String,
}

/// Add a new auth token for the current user
#[utoipa::path(
    post,
    path = "/me/tokens",
    tag = "users",
    request_body = token::NewTokenReqData,
    responses(
        (status = 200, description = "Token created successfully", body = NewTokenResponse),
        (status = 400, description = "Invalid token name"),
        (status = 401, description = "Not authenticated")
    ),
    security(("session_cookie" = []))
)]
pub async fn add_token(
    user: SessionUser,
    State(db): DbState,
    State(cache): TokenCacheState,
    Json(auth_token): Json<token::NewTokenReqData>,
) -> Result<Json<NewTokenResponse>, ApiError> {
    validate_token_name(&auth_token.name)?;
    let token = token::generate_token();
    db.add_auth_token(&auth_token.name, &token, &user.name)
        .await?;

    cache.invalidate_all();

    Ok(NewTokenResponse {
        name: auth_token.name.clone(),
        token: token.into_secret(),
    }
    .into())
}

/// List auth tokens for the current user
#[utoipa::path(
    get,
    path = "/me/tokens",
    tag = "users",
    responses(
        (status = 200, description = "List of auth tokens", body = Vec<AuthToken>),
        (status = 401, description = "Not authenticated")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_tokens(
    user: SessionUser,
    State(db): DbState,
) -> Result<Json<Vec<AuthToken>>, ApiError> {
    Ok(Json(db.get_auth_tokens(&user.name).await?))
}

/// List all users (admin only)
#[utoipa::path(
    get,
    path = "/",
    tag = "users",
    responses(
        (status = 200, description = "List of all users", body = Vec<User>),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_users(_user: AdminUser, State(db): DbState) -> Result<Json<Vec<User>>, ApiError> {
    Ok(Json(db.get_users().await?))
}

/// Delete an auth token
#[utoipa::path(
    delete,
    path = "/me/tokens/{id}",
    tag = "users",
    params(
        ("id" = i64, Path, description = "Token ID to delete")
    ),
    responses(
        (status = 200, description = "Token deleted successfully"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Token not found")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_token(
    user: SessionUser,
    Path(id): Path<AuthTokenId>,
    State(db): DbState,
    State(cache): TokenCacheState,
) -> Result<(), ApiError> {
    // Only deletes a token of the user
    db.delete_auth_token(&user.name, id).await?;

    cache.invalidate_all();

    Ok(())
}

#[derive(Serialize, ToSchema)]
pub struct ResetPwd {
    new_pwd: String,
    user: UserName,
}

/// Reset a user's password (admin only)
#[utoipa::path(
    put,
    path = "/{name}/password",
    tag = "users",
    params(
        ("name" = String, Path, description = "Username")
    ),
    responses(
        (status = 200, description = "Password reset successfully", body = ResetPwd),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn reset_pwd(
    admin: AdminUser,
    jar: PrivateCookieJar,
    Path(name): Path<UserName>,
    State(db): DbState,
    State(cache): TokenCacheState,
    State(settings): SettingsState,
) -> Result<Json<ResetPwd>, ApiError> {
    ensure_local_auth_enabled(&settings)?;

    let new_pwd = generate_rand_string(12);
    db.change_pwd(&name, &new_pwd).await?;

    // An admin resetting their own password stays logged in.
    let keep_session = jar.get(COOKIE_SESSION_ID).filter(|_| admin.name == name);
    revoke_sessions(db.as_ref(), &name, keep_session.as_ref().map(Cookie::value)).await?;

    // Drop cached basic auth credentials carrying the old password.
    cache.invalidate_all();

    // Report the user whose password was reset, not the admin performing it.
    Ok(ResetPwd {
        user: name,
        new_pwd,
    }
    .into())
}

#[derive(Deserialize, ToSchema)]
pub struct ReadOnlyState {
    pub state: bool,
}

/// Change a user's read-only state (admin only)
#[utoipa::path(
    post,
    path = "/{name}/read-only",
    tag = "users",
    params(
        ("name" = String, Path, description = "Username")
    ),
    request_body = ReadOnlyState,
    responses(
        (status = 200, description = "Read-only state changed successfully"),
        (status = 400, description = "Cannot lock yourself"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn read_only(
    user: AdminUser,
    Path(name): Path<UserName>,
    State(db): DbState,
    State(cache): TokenCacheState,
    Json(ro_state): Json<ReadOnlyState>,
) -> Result<(), ApiError> {
    // Prevent self-locking to avoid lockout
    if user.name == name && ro_state.state {
        return Err(ApiError::bad_request("You cannot make yourself read-only"));
    }

    db.change_read_only_state(&name, ro_state.state).await?;

    cache.invalidate_all();

    Ok(())
}

#[derive(Deserialize, ToSchema)]
pub struct AdminState {
    pub state: bool,
}

/// Change a user's admin state (admin only)
#[utoipa::path(
    post,
    path = "/{name}/admin",
    tag = "users",
    params(
        ("name" = String, Path, description = "Username")
    ),
    request_body = AdminState,
    responses(
        (status = 200, description = "Admin state changed successfully"),
        (status = 400, description = "Cannot demote yourself"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn admin(
    user: AdminUser,
    Path(name): Path<UserName>,
    State(db): DbState,
    State(cache): TokenCacheState,
    Json(admin_state): Json<AdminState>,
) -> Result<(), ApiError> {
    // Prevent self-demotion to avoid lockout
    if user.name == name && !admin_state.state {
        return Err(ApiError::bad_request(
            "You cannot remove your own admin rights",
        ));
    }

    db.change_admin_state(&name, admin_state.state).await?;

    cache.invalidate_all();

    Ok(())
}

/// Delete a user (admin only)
#[utoipa::path(
    delete,
    path = "/{name}",
    tag = "users",
    params(
        ("name" = String, Path, description = "Username to delete")
    ),
    responses(
        (status = 200, description = "User deleted successfully"),
        (status = 400, description = "Cannot delete yourself"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete(
    user: AdminUser,
    Path(name): Path<UserName>,
    State(db): DbState,
    State(cache): TokenCacheState,
) -> Result<(), ApiError> {
    // Prevent self-deletion to avoid lockout
    if user.name == name {
        return Err(ApiError::bad_request("You cannot delete yourself"));
    }

    db.delete_user(&name).await?;

    cache.invalidate_all();

    Ok(())
}

#[derive(Serialize, ToSchema)]
pub struct LoggedInUser {
    user: String,
    is_admin: bool,
    is_logged_in: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct Credentials {
    pub user: String,
    pub pwd: String,
}

impl Credentials {
    /// Returns the name of the user to log in.
    pub fn validate(&self) -> Result<UserName, ApiError> {
        let name = UserName::new(self.user.as_str())?;
        if self.pwd.is_empty() {
            return Err(ApiError::bad_request("The password must not be empty"));
        }
        Ok(name)
    }
}

/// Login with username and password
#[utoipa::path(
    post,
    path = "/login",
    tag = "auth",
    request_body = Credentials,
    responses(
        (status = 200, description = "Successfully logged in", body = LoggedInUser),
        (status = 400, description = "Invalid credentials"),
        (status = 401, description = "Authentication failed")
    )
)]
pub async fn login(
    cookies: PrivateCookieJar,
    State(state): AppState,
    Json(credentials): Json<Credentials>,
) -> Result<(PrivateCookieJar, Json<LoggedInUser>), ApiError> {
    ensure_local_auth_enabled(&state.settings)?;
    let name = credentials.validate()?;

    let user = authenticate_password(
        state.db.as_ref(),
        &state.login_throttle,
        &name,
        &credentials.pwd,
    )
    .await
    .map_err(|e| match e {
        PasswordAuthError::Throttled => ApiError::from(StatusCode::TOO_MANY_REQUESTS),
        PasswordAuthError::Invalid => {
            warn!("Login with wrong username or password");
            ApiError::unauthorized("Wrong username or password")
        }
    })?;

    let jar = create_session_jar(cookies, &state, &name).await?;

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

/// Get current login state
#[utoipa::path(
    get,
    path = "/state",
    tag = "auth",
    responses(
        (status = 200, description = "Current login state", body = LoggedInUser)
    )
)]
#[expect(clippy::unused_async)] // part of the router
pub async fn login_state(user: Option<SessionUser>) -> Json<LoggedInUser> {
    match user {
        Some(SessionUser(user)) => LoggedInUser {
            user: user.name.into_inner(),
            is_admin: user.is_admin,
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

#[derive(Serialize, ToSchema)]
pub struct LogoutResponse {
    pub logout_url: Option<String>,
}

/// Logout and clear session
#[utoipa::path(
    post,
    path = "/logout",
    tag = "auth",
    responses(
        (status = 200, description = "Successfully logged out", body = LogoutResponse)
    )
)]
pub async fn logout(
    mut jar: PrivateCookieJar,
    State(state): AppState,
    Extension(oauth2_handler): OAuth2Ext,
) -> Result<(PrivateCookieJar, Json<LogoutResponse>), ApiError> {
    let session_id = match jar.get(COOKIE_SESSION_ID) {
        Some(c) => c.value().to_owned(),
        None => return Ok((jar, Json(LogoutResponse { logout_url: None }))),
    };

    let logout_url = jar
        .get(COOKIE_OIDC_ID_TOKEN)
        .zip(oauth2_handler.as_ref())
        .and_then(|(id_token, handler)| {
            let post_logout = format!("{}/", state.settings.origin.base_url());
            handler.end_session_url(id_token.value(), &post_logout)
        });

    jar = jar.remove(Cookie::build((COOKIE_SESSION_ID, "")).path("/"));
    jar = jar.remove(Cookie::build((COOKIE_SESSION_USER, "")).path("/"));
    jar = jar.remove(Cookie::build((COOKIE_OIDC_ID_TOKEN, "")).path("/"));

    // The browser drops the cookie even if the session cannot be deleted, so
    // the jar must still be returned.
    if let Err(e) = state
        .db
        .delete_session_token(&PlainToken::from(session_id))
        .await
    {
        tracing::error!("Failed to delete session on logout: {e}");
    }
    Ok((jar, Json(LogoutResponse { logout_url })))
}

#[derive(Deserialize, ToSchema)]
pub struct PwdChange {
    pub old_pwd: String,
    pub new_pwd1: String,
    pub new_pwd2: String,
}

impl PwdChange {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.old_pwd.is_empty() {
            return Err(ApiError::bad_request("The old password must not be empty"));
        }
        if self.new_pwd1.is_empty() || self.new_pwd2.is_empty() {
            return Err(ApiError::bad_request("The new password must not be empty"));
        }
        if self.new_pwd1 != self.new_pwd2 {
            return Err(ApiError::bad_request("The new passwords do not match"));
        }
        Ok(())
    }
}

/// Change the current user's password
#[utoipa::path(
    put,
    path = "/me/password",
    tag = "users",
    request_body = PwdChange,
    responses(
        (status = 200, description = "Password changed successfully"),
        (status = 400, description = "Invalid password or validation failed"),
        (status = 401, description = "Not authenticated")
    ),
    security(("session_cookie" = []))
)]
pub async fn change_pwd(
    user: SessionUser,
    jar: PrivateCookieJar,
    State(db): DbState,
    State(cache): TokenCacheState,
    State(throttle): LoginThrottleState,
    State(settings): SettingsState,
    Json(pwd_change): Json<PwdChange>,
) -> Result<(), ApiError> {
    ensure_local_auth_enabled(&settings)?;
    pwd_change.validate()?;

    let user = authenticate_password(db.as_ref(), &throttle, &user.name, &pwd_change.old_pwd)
        .await
        .map_err(|e| match e {
            PasswordAuthError::Throttled => ApiError::from(StatusCode::TOO_MANY_REQUESTS),
            PasswordAuthError::Invalid => ApiError::bad_request("The old password is wrong"),
        })?;

    db.change_pwd(&user.name, &pwd_change.new_pwd1).await?;
    let current_session = jar.get(COOKIE_SESSION_ID);
    revoke_sessions(
        db.as_ref(),
        &user.name,
        current_session.as_ref().map(Cookie::value),
    )
    .await?;

    // Drop cached basic auth credentials carrying the old password.
    cache.invalidate_all();
    Ok(())
}

#[derive(Deserialize, ToSchema)]
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
    /// Returns the name of the new user.
    pub fn validate(&self) -> Result<UserName, ApiError> {
        let name = UserName::for_new_user(self.name.as_str())?;
        if self.pwd1.is_empty() || self.pwd2.is_empty() {
            return Err(ApiError::bad_request("The password must not be empty"));
        }
        if self.pwd1 != self.pwd2 {
            return Err(ApiError::bad_request("The passwords do not match"));
        }
        Ok(name)
    }
}

/// Create a new user (admin only)
#[utoipa::path(
    post,
    path = "/",
    tag = "users",
    request_body = NewUser,
    responses(
        (status = 200, description = "User created successfully"),
        (status = 400, description = "Validation failed"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn add(
    _user: AdminUser,
    State(db): DbState,
    State(cache): TokenCacheState,
    State(settings): SettingsState,
    Json(new_user): Json<NewUser>,
) -> Result<(), ApiError> {
    ensure_local_auth_enabled(&settings)?;
    let name = new_user.validate()?;

    db.add_user(
        &name,
        &new_user.pwd1,
        UserFlags {
            admin: new_user.is_admin,
            read_only: new_user.is_read_only,
        },
    )
    .await?;

    cache.invalidate_all();

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::Router;
    use axum::body::Body;
    use axum::http::Response;
    use axum::response::IntoResponse;
    use axum::routing::{post, put};
    use hyper::{Method, Request, header};
    use kellnr_db::test_utils::test_drop_table;
    use kellnr_http::login_throttle::LoginThrottle;
    use kellnr_http::state::AppStateData;
    use kellnr_http::test_support::{encode_cookies, session_cookie, sqlite_test_state};
    use kellnr_http::token_cache::{CachedTokenData, TokenCacheManager};
    use kellnr_settings::constants::COOKIE_SESSION_ID;
    use kellnr_test_db::TestDb;
    use tower::ServiceExt;

    use super::*;

    /// The user routes at their paths in the API.
    fn app(state: AppStateData) -> Router {
        Router::new()
            .route("/login", post(login))
            .route("/logout", post(logout))
            .route("/", post(add))
            .route("/{name}", axum::routing::delete(delete))
            .route("/{name}/password", put(reset_pwd))
            .route("/{name}/admin", post(admin))
            .route("/{name}/read-only", post(read_only))
            .route("/me/password", put(change_pwd))
            .route("/me/tokens", post(add_token))
            .route("/me/tokens/{id}", axum::routing::delete(delete_token))
            .layer(Extension::<Option<Arc<kellnr_oauth2::OAuth2Handler>>>(None))
            .with_state(state)
    }

    fn sso_enforced(mut state: AppStateData) -> AppStateData {
        let mut settings = (*state.settings).clone();
        settings.oauth2.enabled = true;
        settings.oauth2.enforced = true;
        state.settings = Arc::new(settings);
        state
    }

    fn token_cache() -> Arc<TokenCacheManager> {
        Arc::new(TokenCacheManager::new(true, Duration::from_secs(60), 100))
    }

    /// A token cache that holds `key` for `user`.
    async fn token_cache_with(key: &str, user: &str) -> Arc<TokenCacheManager> {
        let cache = token_cache();
        cache
            .insert(
                key.to_string(),
                CachedTokenData {
                    user: UserName::new(user).unwrap(),
                    is_admin: false,
                    is_read_only: false,
                },
                cache.generation(),
            )
            .await;
        cache
    }

    async fn send(
        state: AppStateData,
        method: Method,
        uri: &str,
        cookie: &str,
        json: Option<&str>,
    ) -> Response<Body> {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, cookie)
            .header(header::CONTENT_TYPE, "application/json");
        let body = json.map_or_else(Body::empty, |json| Body::from(json.to_string()));
        app(state)
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap()
    }

    async fn add_user(db: &TestDb, name: &str, flags: UserFlags) {
        db.add_user(&UserName::new(name).unwrap(), "pwd", flags)
            .await
            .unwrap();
    }

    async fn has_password(db: &TestDb, name: &str, pwd: &str) -> bool {
        db.authenticate_user(&UserName::new(name).unwrap(), pwd)
            .await
            .is_ok()
    }

    async fn user(db: &TestDb, name: &str) -> User {
        db.get_user(&UserName::new(name).unwrap()).await.unwrap()
    }

    async fn has_session(db: &TestDb, token: &str) -> bool {
        db.validate_session(&PlainToken::from(token)).await.is_ok()
    }

    async fn add_session(db: &TestDb, name: &str, token: &str) {
        db.add_session_token(&UserName::new(name).unwrap(), &PlainToken::from(token))
            .await
            .unwrap();
    }

    // ==========================
    // SSO Enforcement Tests
    // ==========================

    /// Password login must be refused outright, so that a still-valid local
    /// password cannot be used to bypass SSO.
    #[tokio::test]
    async fn test_login_forbidden_when_sso_enforced() {
        let (state, _db) = sqlite_test_state().await;

        let response = send(
            sso_enforced(state),
            Method::POST,
            "/login",
            "",
            Some(r#"{"user":"admin","pwd":"123"}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    /// Changing one's own password is meaningless under SSO and must not write
    /// a new hash to the DB.
    #[tokio::test]
    async fn test_change_pwd_forbidden_when_sso_enforced() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;

        let response = send(
            sso_enforced(state),
            Method::PUT,
            "/me/password",
            &cookie,
            Some(r#"{"old_pwd":"pwd","new_pwd1":"newpassword","new_pwd2":"newpassword"}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(has_password(&db, "user", "pwd").await);
    }

    /// An admin resetting another user's password must not produce a usable
    /// local credential while SSO is the only permitted login method.
    #[tokio::test]
    async fn test_reset_pwd_forbidden_when_sso_enforced() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, "someuser", UserFlags::normal()).await;

        let response = send(
            sso_enforced(state),
            Method::PUT,
            "/someuser/password",
            &cookie,
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(has_password(&db, "someuser", "pwd").await);
    }

    /// Users are provisioned by the `IdP` under enforcement, so creating a local
    /// password-backed account must be refused.
    #[tokio::test]
    async fn test_add_user_forbidden_when_sso_enforced() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let response = send(
            sso_enforced(state),
            Method::POST,
            "/",
            &cookie,
            Some(
                r#"{"name":"new_user","pwd1":"password","pwd2":"password","is_admin":false,"is_read_only":false}"#,
            ),
        )
        .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(
            db.get_user(&UserName::new("new_user").unwrap())
                .await
                .is_err()
        );
    }

    async fn post_login(state: &AppStateData, user: &str) -> StatusCode {
        send(
            state.clone(),
            Method::POST,
            "/login",
            "",
            Some(&format!(r#"{{"user":"{user}","pwd":"guess"}}"#)),
        )
        .await
        .status()
    }

    /// Password guessing on the login form is throttled per username. Other
    /// users stay unaffected so a single attacker cannot lock out everyone.
    #[tokio::test]
    async fn test_login_throttled_after_failed_attempts() {
        let (state, db) = sqlite_test_state().await;
        add_user(&db, "other", UserFlags::normal()).await;
        let state = AppStateData {
            login_throttle: Arc::new(LoginThrottle::new(2, Duration::from_mins(5))),
            ..state
        };

        assert_eq!(post_login(&state, "admin").await, StatusCode::UNAUTHORIZED);
        assert_eq!(post_login(&state, "admin").await, StatusCode::UNAUTHORIZED);
        assert_eq!(
            post_login(&state, "admin").await,
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(post_login(&state, "other").await, StatusCode::UNAUTHORIZED);
    }

    /// The old password check on password change must not become a way
    /// around the login throttle.
    #[tokio::test]
    async fn test_change_pwd_throttled_after_failed_attempts() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        let state = AppStateData {
            login_throttle: Arc::new(LoginThrottle::new(1, Duration::from_mins(5))),
            ..state
        };

        let mut statuses = Vec::new();
        for _ in 0..2 {
            let response = send(
                state.clone(),
                Method::PUT,
                "/me/password",
                &cookie,
                Some(r#"{"old_pwd":"guess","new_pwd1":"newpassword","new_pwd2":"newpassword"}"#),
            )
            .await;
            statuses.push(response.status());
        }

        assert_eq!(
            statuses,
            [StatusCode::BAD_REQUEST, StatusCode::TOO_MANY_REQUESTS]
        );
        assert!(has_password(&db, "user", "pwd").await);
    }

    /// The removal cookie must carry the same path as the session cookie,
    /// otherwise the browser keeps the original cookie after logout.
    #[tokio::test]
    async fn test_logout_removes_session_cookie_with_root_path() {
        let (state, db) = sqlite_test_state().await;
        add_session(&db, "admin", "session").await;

        let response = send(
            state,
            Method::POST,
            "/logout",
            &encode_cookies([(COOKIE_SESSION_ID, "session")]),
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let removal = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .find(|c| c.starts_with(COOKIE_SESSION_ID))
            .expect("session cookie must be removed");
        assert!(removal.contains("Path=/"), "{removal}");
        assert!(removal.contains("Max-Age=0"), "{removal}");
        assert!(!has_session(&db, "session").await);
    }

    #[tokio::test]
    async fn test_logout_removes_session_cookie_if_session_cannot_be_deleted() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        test_drop_table(&db, "session").await.unwrap();

        let response = send(state, Method::POST, "/logout", &cookie, None).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .any(|v| v.to_str().unwrap().starts_with(COOKIE_SESSION_ID))
        );
    }

    fn new_user(name: &str) -> NewUser {
        NewUser {
            pwd1: "pwd".to_string(),
            pwd2: "pwd".to_string(),
            name: name.to_string(),
            is_admin: false,
            is_read_only: false,
        }
    }

    /// Basic auth splits at the first ':', so such a user could never log in.
    #[tokio::test]
    async fn test_add_user_with_invalid_name_is_bad_request() {
        let db = TestDb::sqlite().await;
        for name in ["", "  ", "a:b", "line\nbreak", &"a".repeat(65)] {
            let result = add(
                AdminUser(kellnr_http::test_support::session_identity("admin", true)),
                State(db.provider()),
                State(token_cache()),
                State(Arc::new(Settings::default())),
                Json(new_user(name)),
            )
            .await;

            assert!(
                result
                    .as_ref()
                    .is_err_and(|e| e.status() == StatusCode::BAD_REQUEST),
                "{name:?}: {result:?}"
            );
        }
        assert_eq!(db.get_users().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_add_token_with_invalid_name_is_bad_request() {
        let db = TestDb::sqlite().await;
        let admin = UserName::new("admin").unwrap();
        let tokens_before = db.get_auth_tokens(&admin).await.unwrap().len();
        for name in [String::new(), "a".repeat(129)] {
            let result = add_token(
                SessionUser(kellnr_http::test_support::session_identity("admin", false)),
                State(db.provider()),
                State(token_cache()),
                Json(token::NewTokenReqData { name }),
            )
            .await;

            assert!(
                result
                    .as_ref()
                    .is_err_and(|e| e.status() == StatusCode::BAD_REQUEST)
            );
        }
        assert_eq!(
            db.get_auth_tokens(&admin).await.unwrap().len(),
            tokens_before
        );
    }

    #[tokio::test]
    async fn test_add_existing_user_is_conflict() {
        let db = TestDb::sqlite().await;
        let cache = token_cache();
        let settings = Arc::new(Settings::default());
        let add_bob = || {
            add(
                AdminUser(kellnr_http::test_support::session_identity("admin", true)),
                State(db.provider()),
                State(cache.clone()),
                State(settings.clone()),
                Json(new_user("bob")),
            )
        };

        add_bob().await.unwrap();
        let response = add_bob().await.unwrap_err().into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    const CHANGE_PWD: &str =
        r#"{"old_pwd":"pwd","new_pwd1":"newpassword","new_pwd2":"newpassword"}"#;

    /// Cached basic auth credentials carry the old password and must not
    /// survive a password change.
    #[tokio::test]
    async fn test_change_pwd_invalidates_cache() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        let cache = token_cache_with("basic:credentials", "user").await;
        let state = AppStateData {
            token_cache: cache.clone(),
            ..state
        };

        let response = send(
            state,
            Method::PUT,
            "/me/password",
            &cookie,
            Some(CHANGE_PWD),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(has_password(&db, "user", "newpassword").await);
        assert!(cache.get("basic:credentials").await.is_none());
    }

    #[tokio::test]
    async fn test_reset_pwd_invalidates_cache() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, "user", UserFlags::normal()).await;
        let cache = token_cache_with("basic:credentials", "user").await;
        let state = AppStateData {
            token_cache: cache.clone(),
            ..state
        };

        let response = send(state, Method::PUT, "/user/password", &cookie, None).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!has_password(&db, "user", "pwd").await);
        assert!(cache.get("basic:credentials").await.is_none());
    }

    /// Changing the password logs out every other session, for example one
    /// an attacker obtained with the old password, but keeps the current one.
    #[tokio::test]
    async fn test_change_pwd_revokes_other_sessions() {
        let (state, db) = sqlite_test_state().await;
        add_user(&db, "user", UserFlags::normal()).await;
        add_session(&db, "user", "current").await;
        add_session(&db, "user", "other").await;

        let response = send(
            state,
            Method::PUT,
            "/me/password",
            &encode_cookies([(COOKIE_SESSION_ID, "current")]),
            Some(CHANGE_PWD),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(has_session(&db, "current").await);
        assert!(!has_session(&db, "other").await);
    }

    /// An admin reset is typically a response to a compromised account, so
    /// all sessions of that user end.
    #[tokio::test]
    async fn test_reset_pwd_revokes_all_sessions_of_user() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, "user", UserFlags::normal()).await;
        add_session(&db, "user", "user-session").await;

        let response = send(state, Method::PUT, "/user/password", &cookie, None).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!has_session(&db, "user-session").await);
    }

    #[tokio::test]
    async fn test_reset_own_pwd_keeps_current_session() {
        let (state, db) = sqlite_test_state().await;
        add_session(&db, "admin", "current").await;
        add_session(&db, "admin", "other").await;

        let response = send(
            state,
            Method::PUT,
            "/admin/password",
            &encode_cookies([(COOKIE_SESSION_ID, "current")]),
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(has_session(&db, "current").await);
        assert!(!has_session(&db, "other").await);
    }

    #[tokio::test]
    async fn test_add_token_invalidates_cache() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "test_user", UserFlags::normal()).await;
        let cache = token_cache_with("existing_token", "test_user").await;
        let state = AppStateData {
            token_cache: cache.clone(),
            ..state
        };

        let response = send(
            state,
            Method::POST,
            "/me/tokens",
            &cookie,
            Some(r#"{"name":"new_token"}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let tokens = db
            .get_auth_tokens(&UserName::new("test_user").unwrap())
            .await
            .unwrap();
        assert_eq!(
            tokens.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["new_token"]
        );
        assert!(cache.get("existing_token").await.is_none());
    }

    /// Adds an API token for `user` and returns its id.
    async fn add_auth_token(db: &TestDb, user: &str) -> AuthTokenId {
        let user = UserName::new(user).unwrap();
        db.add_auth_token("ci", &PlainToken::from(format!("{user}-token")), &user)
            .await
            .unwrap();
        db.get_auth_tokens(&user).await.unwrap()[0].id
    }

    #[tokio::test]
    async fn test_delete_token_invalidates_cache() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "test_user", UserFlags::normal()).await;
        let id = add_auth_token(&db, "test_user").await;
        let cache = token_cache_with("token_to_keep", "test_user").await;
        let state = AppStateData {
            token_cache: cache.clone(),
            ..state
        };

        let response = send(
            state,
            Method::DELETE,
            &format!("/me/tokens/{id}"),
            &cookie,
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let user = UserName::new("test_user").unwrap();
        assert!(db.get_auth_tokens(&user).await.unwrap().is_empty());
        assert!(cache.get("token_to_keep").await.is_none());
    }

    #[tokio::test]
    async fn test_delete_token_of_other_user_is_not_found() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "test_user", UserFlags::normal()).await;
        add_user(&db, "other", UserFlags::normal()).await;
        let id = add_auth_token(&db, "other").await;

        let response = send(
            state,
            Method::DELETE,
            &format!("/me/tokens/{id}"),
            &cookie,
            None,
        )
        .await;

        assert_eq!(StatusCode::NOT_FOUND, response.status());
        let other = UserName::new("other").unwrap();
        assert_eq!(db.get_auth_tokens(&other).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_delete_user_invalidates_cache() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, "user_to_delete", UserFlags::normal()).await;
        let cache = token_cache_with("user_token", "user_to_delete").await;
        let state = AppStateData {
            token_cache: cache.clone(),
            ..state
        };

        let response = send(state, Method::DELETE, "/user_to_delete", &cookie, None).await;

        assert_eq!(response.status(), StatusCode::OK);
        let name = UserName::new("user_to_delete").unwrap();
        assert!(db.get_user(&name).await.is_err());
        assert!(cache.get("user_token").await.is_none());
    }

    #[tokio::test]
    async fn test_read_only_change_invalidates_cache() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, "target_user", UserFlags::normal()).await;
        let cache = token_cache_with("user_token", "target_user").await;
        let state = AppStateData {
            token_cache: cache.clone(),
            ..state
        };

        let response = send(
            state,
            Method::POST,
            "/target_user/read-only",
            &cookie,
            Some(r#"{"state":true}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(user(&db, "target_user").await.is_read_only);
        // The cached permissions are outdated now.
        assert!(cache.get("user_token").await.is_none());
    }

    #[tokio::test]
    async fn test_admin_self_locking_prevented() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let response = send(
            state,
            Method::POST,
            "/admin/read-only",
            &cookie,
            Some(r#"{"state":true}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!user(&db, "admin").await.is_read_only);
    }

    /// Only locking oneself is blocked, unlocking is allowed.
    #[tokio::test]
    async fn test_admin_self_unlocking_allowed() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let response = send(
            state,
            Method::POST,
            "/admin/read-only",
            &cookie,
            Some(r#"{"state":false}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!user(&db, "admin").await.is_read_only);
    }

    #[tokio::test]
    async fn test_admin_locking_other_user_works() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, "other_user", UserFlags::normal()).await;

        let response = send(
            state,
            Method::POST,
            "/other_user/read-only",
            &cookie,
            Some(r#"{"state":true}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(user(&db, "other_user").await.is_read_only);
    }

    #[tokio::test]
    async fn test_add_user_invalidates_cache() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        let cache = token_cache_with("existing_token", "existing_user").await;
        let state = AppStateData {
            token_cache: cache.clone(),
            ..state
        };

        let response = send(
            state,
            Method::POST,
            "/",
            &cookie,
            Some(
                r#"{"name":"new_user","pwd1":"password","pwd2":"password","is_admin":false,"is_read_only":false}"#,
            ),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(has_password(&db, "new_user", "password").await);
        assert!(cache.get("existing_token").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_not_invalidated_on_db_failure() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "test_user", UserFlags::normal()).await;
        test_drop_table(&db, "auth_token").await.unwrap();
        let cache = token_cache_with("existing_token", "test_user").await;
        let state = AppStateData {
            token_cache: cache.clone(),
            ..state
        };

        let response = send(
            state,
            Method::POST,
            "/me/tokens",
            &cookie,
            Some(r#"{"name":"new_token"}"#),
        )
        .await;

        assert!(!response.status().is_success());
        assert!(cache.get("existing_token").await.is_some());
    }

    #[tokio::test]
    async fn test_admin_change_invalidates_cache() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, "target_user", UserFlags::normal()).await;
        let cache = token_cache_with("user_token", "target_user").await;
        let state = AppStateData {
            token_cache: cache.clone(),
            ..state
        };

        let response = send(
            state,
            Method::POST,
            "/target_user/admin",
            &cookie,
            Some(r#"{"state":true}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(user(&db, "target_user").await.is_admin);
        // The cached permissions are outdated now.
        assert!(cache.get("user_token").await.is_none());
    }

    #[tokio::test]
    async fn test_admin_self_demotion_prevented() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let response = send(
            state,
            Method::POST,
            "/admin/admin",
            &cookie,
            Some(r#"{"state":false}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(user(&db, "admin").await.is_admin);
    }

    #[tokio::test]
    async fn test_non_admin_cannot_change_admin_status() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "regular_user", UserFlags::normal()).await;
        add_user(&db, "target_user", UserFlags::normal()).await;

        let response = send(
            state,
            Method::POST,
            "/target_user/admin",
            &cookie,
            Some(r#"{"state":true}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!user(&db, "target_user").await.is_admin);
    }

    #[tokio::test]
    async fn test_admin_demotion_works() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, "other_admin", UserFlags::admin()).await;

        let response = send(
            state,
            Method::POST,
            "/other_admin/admin",
            &cookie,
            Some(r#"{"state":false}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!user(&db, "other_admin").await.is_admin);
    }

    #[tokio::test]
    async fn test_admin_nonexistent_user_returns_error() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let response = send(
            state,
            Method::POST,
            "/nonexistent/admin",
            &cookie,
            Some(r#"{"state":true}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// Only demoting oneself is blocked, promoting is a harmless no-op.
    #[tokio::test]
    async fn test_admin_self_promotion_allowed() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let response = send(
            state,
            Method::POST,
            "/admin/admin",
            &cookie,
            Some(r#"{"state":true}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(user(&db, "admin").await.is_admin);
    }

    #[tokio::test]
    async fn test_admin_self_deletion_prevented() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let response = send(state, Method::DELETE, "/admin", &cookie, None).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        user(&db, "admin").await;
    }

    #[tokio::test]
    async fn test_admin_deletion_of_other_user_works() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, "other_user", UserFlags::normal()).await;

        let response = send(state, Method::DELETE, "/other_user", &cookie, None).await;

        assert_eq!(response.status(), StatusCode::OK);
        let name = UserName::new("other_user").unwrap();
        assert!(db.get_user(&name).await.is_err());
    }

    /// Names stored before the name rules existed must still be usable.
    #[tokio::test]
    async fn test_admin_deletion_of_user_with_name_against_the_rules_works() {
        let legacy_name = "legacy:user";
        assert!(UserName::for_new_user(legacy_name).is_err());
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_user(&db, legacy_name, UserFlags::normal()).await;

        let response = send(
            state,
            Method::DELETE,
            &format!("/{legacy_name}"),
            &cookie,
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let name = UserName::new(legacy_name).unwrap();
        assert!(db.get_user(&name).await.is_err());
    }
}
