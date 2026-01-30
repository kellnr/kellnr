use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::Cookie;
use cookie::time;
use kellnr_appstate::{AppState, DbState, TokenCacheState};
use kellnr_auth::token;
use kellnr_common::util::generate_rand_string;
use kellnr_db::password::generate_salt;
use kellnr_db::{self, AuthToken, User};
use kellnr_settings::constants::{COOKIE_SESSION_ID, COOKIE_SESSION_USER};
use serde::{Deserialize, Serialize};

use crate::error::RouteError;
use crate::session::{AdminUser, MaybeUser};

#[derive(Serialize)]
pub struct NewTokenResponse {
    name: String,
    token: String,
}

pub async fn add_token(
    user: MaybeUser,
    State(db): DbState,
    State(cache): TokenCacheState,
    Json(auth_token): Json<token::NewTokenReqData>,
) -> Result<Json<NewTokenResponse>, RouteError> {
    let token = token::generate_token();
    db.add_auth_token(&auth_token.name, &token, user.name())
        .await?;

    cache.invalidate_all();

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
    _user: AdminUser,
    State(db): DbState,
) -> Result<Json<Vec<User>>, RouteError> {
    Ok(Json(db.get_users().await?))
}

pub async fn delete_token(
    user: MaybeUser,
    Path(id): Path<i32>,
    State(db): DbState,
    State(cache): TokenCacheState,
) -> Result<(), RouteError> {
    db.get_auth_tokens(user.name())
        .await?
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| RouteError::Status(StatusCode::BAD_REQUEST))?;

    db.delete_auth_token(id).await?;

    cache.invalidate_all();

    Ok(())
}

#[derive(Serialize)]
pub struct ResetPwd {
    new_pwd: String,
    user: String,
}

pub async fn reset_pwd(
    user: AdminUser,
    Path(name): Path<String>,
    State(db): DbState,
) -> Result<Json<ResetPwd>, RouteError> {
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
    _user: AdminUser,
    Path(name): Path<String>,
    State(db): DbState,
    State(cache): TokenCacheState,
    Json(ro_state): Json<ReadOnlyState>,
) -> Result<(), RouteError> {
    db.change_read_only_state(&name, ro_state.state).await?;

    cache.invalidate_all();

    Ok(())
}

#[derive(Deserialize)]
pub struct AdminState {
    pub state: bool,
}

pub async fn admin(
    user: AdminUser,
    Path(name): Path<String>,
    State(db): DbState,
    State(cache): TokenCacheState,
    Json(admin_state): Json<AdminState>,
) -> Result<(), RouteError> {
    // Prevent self-demotion to avoid lockout
    if user.name() == name && !admin_state.state {
        return Err(RouteError::Status(StatusCode::BAD_REQUEST));
    }

    db.change_admin_state(&name, admin_state.state).await?;

    cache.invalidate_all();

    Ok(())
}

pub async fn delete(
    _user: AdminUser,
    Path(name): Path<String>,
    State(db): DbState,
    State(cache): TokenCacheState,
) -> Result<(), RouteError> {
    db.delete_user(&name).await?;

    cache.invalidate_all();

    Ok(())
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
    _user: AdminUser,
    State(db): DbState,
    State(cache): TokenCacheState,
    Json(new_user): Json<NewUser>,
) -> Result<(), RouteError> {
    new_user.validate()?;

    let salt = generate_salt();
    db.add_user(
        &new_user.name,
        &new_user.pwd1,
        &salt,
        new_user.is_admin,
        new_user.is_read_only,
    )
    .await?;

    cache.invalidate_all();

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::routing::post;
    use axum_extra::extract::cookie::Key;
    use hyper::{Request, header};
    use kellnr_appstate::AppStateData;
    use kellnr_common::token_cache::{CachedTokenData, TokenCacheManager};
    use kellnr_db::AuthToken;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_settings::constants::COOKIE_SESSION_ID;
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
    use mockall::predicate::*;
    use tower::ServiceExt;

    use super::*;
    use crate::test_helper::{TEST_KEY, encode_cookies};

    fn test_state_with_cache(mock_db: MockDb, cache: Arc<TokenCacheManager>) -> AppStateData {
        let settings = Arc::new(kellnr_settings::test_settings());
        let kellnr_storage =
            Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
        let crate_storage = Arc::new(KellnrCrateStorage::new(&settings, kellnr_storage));
        let cratesio_storage = Arc::new(CratesIoCrateStorage::new(
            &settings,
            Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage,
        ));
        let (cratesio_prefetch_sender, _) = flume::unbounded();

        AppStateData {
            db: Arc::new(mock_db),
            signing_key: Key::from(TEST_KEY),
            settings,
            crate_storage,
            cratesio_storage,
            cratesio_prefetch_sender,
            token_cache: cache,
            toolchain_storage: None,
        }
    }

    #[tokio::test]
    async fn test_add_token_invalidates_cache() {
        // Pre-populate cache with a token
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));
        cache
            .insert(
                "existing_token".to_string(),
                CachedTokenData {
                    user: "test_user".to_string(),
                    is_admin: false,
                    is_read_only: false,
                },
            )
            .await;

        // Verify token is cached
        assert!(cache.get("existing_token").await.is_some());

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("test_user".to_string(), false)));
        mock_db
            .expect_add_auth_token()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/add_token", post(add_token))
            .with_state(state);

        let response = app
            .oneshot(
                Request::post("/add_token")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"name":"new_token"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status().is_success(),
            "Expected success but got {}",
            response.status()
        );

        // Verify cache was invalidated
        assert!(cache.get("existing_token").await.is_none());
    }

    #[tokio::test]
    async fn test_delete_token_invalidates_cache() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));
        cache
            .insert(
                "token_to_keep".to_string(),
                CachedTokenData {
                    user: "test_user".to_string(),
                    is_admin: false,
                    is_read_only: false,
                },
            )
            .await;

        assert!(cache.get("token_to_keep").await.is_some());

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("test_user".to_string(), false)));
        mock_db.expect_get_auth_tokens().times(1).returning(|_| {
            Ok(vec![AuthToken::new(
                1,
                "token".to_string(),
                "secret".to_string(),
            )])
        });
        mock_db
            .expect_delete_auth_token()
            .times(1)
            .with(eq(1))
            .returning(|_| Ok(()));

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/delete_token/{id}", axum::routing::delete(delete_token))
            .with_state(state);

        let response = app
            .oneshot(
                Request::delete("/delete_token/1")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status().is_success(),
            "Expected success but got {}",
            response.status()
        );

        // Verify cache was invalidated
        assert!(cache.get("token_to_keep").await.is_none());
    }

    #[tokio::test]
    async fn test_delete_user_invalidates_cache() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));
        cache
            .insert(
                "user_token".to_string(),
                CachedTokenData {
                    user: "user_to_delete".to_string(),
                    is_admin: false,
                    is_read_only: false,
                },
            )
            .await;

        assert!(cache.get("user_token").await.is_some());

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("admin".to_string(), true))); // Must be admin
        mock_db
            .expect_delete_user()
            .times(1)
            .with(eq("user_to_delete"))
            .returning(|_| Ok(()));

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/delete/{name}", axum::routing::delete(delete))
            .with_state(state);

        let response = app
            .oneshot(
                Request::delete("/delete/user_to_delete")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status().is_success(),
            "Expected success but got {}",
            response.status()
        );

        // Verify cache was invalidated
        assert!(cache.get("user_token").await.is_none());
    }

    #[tokio::test]
    async fn test_read_only_change_invalidates_cache() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));
        cache
            .insert(
                "user_token".to_string(),
                CachedTokenData {
                    user: "target_user".to_string(),
                    is_admin: false,
                    is_read_only: false, // Currently NOT read-only
                },
            )
            .await;

        assert!(cache.get("user_token").await.is_some());

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("admin".to_string(), true))); // Must be admin
        mock_db
            .expect_change_read_only_state()
            .times(1)
            .with(eq("target_user"), eq(true))
            .returning(|_, _| Ok(()));

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/read_only/{name}", post(read_only))
            .with_state(state);

        let response = app
            .oneshot(
                Request::post("/read_only/target_user")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"state":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status().is_success(),
            "Expected success but got {}",
            response.status()
        );

        // Verify cache was invalidated - important because is_read_only permission changed
        assert!(cache.get("user_token").await.is_none());
    }

    #[tokio::test]
    async fn test_add_user_invalidates_cache() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));
        cache
            .insert(
                "existing_token".to_string(),
                CachedTokenData {
                    user: "existing_user".to_string(),
                    is_admin: false,
                    is_read_only: false,
                },
            )
            .await;

        assert!(cache.get("existing_token").await.is_some());

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("admin".to_string(), true))); // Must be admin
        mock_db
            .expect_add_user()
            .times(1)
            .returning(|_, _, _, _, _| Ok(()));

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new().route("/add", post(add)).with_state(state);

        let response = app
            .oneshot(
                Request::post("/add")
                    .header(header::COOKIE, encode_cookies([(COOKIE_SESSION_ID, "session")]))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"name":"new_user","pwd1":"password","pwd2":"password","is_admin":false,"is_read_only":false}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status().is_success(),
            "Expected success but got {}",
            response.status()
        );

        // Verify cache was invalidated
        assert!(cache.get("existing_token").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_not_invalidated_on_db_failure() {
        // Verify cache is NOT invalidated when DB operation fails
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));
        cache
            .insert(
                "existing_token".to_string(),
                CachedTokenData {
                    user: "test_user".to_string(),
                    is_admin: false,
                    is_read_only: false,
                },
            )
            .await;

        assert!(cache.get("existing_token").await.is_some());

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("test_user".to_string(), false)));
        mock_db
            .expect_add_auth_token()
            .times(1)
            .returning(|_, _, _| {
                Err(DbError::InitializationError(
                    "Connection timeout".to_string(),
                ))
            });

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/add_token", post(add_token))
            .with_state(state);

        let response = app
            .oneshot(
                Request::post("/add_token")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"name":"new_token"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Request should have failed
        assert!(!response.status().is_success());

        // Cache should still contain the token since operation failed
        assert!(cache.get("existing_token").await.is_some());
    }

    #[tokio::test]
    async fn test_admin_change_invalidates_cache() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));
        cache
            .insert(
                "user_token".to_string(),
                CachedTokenData {
                    user: "target_user".to_string(),
                    is_admin: false, // Currently NOT admin
                    is_read_only: false,
                },
            )
            .await;

        assert!(cache.get("user_token").await.is_some());

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("admin".to_string(), true))); // Must be admin
        mock_db
            .expect_change_admin_state()
            .times(1)
            .with(eq("target_user"), eq(true))
            .returning(|_, _| Ok(()));

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/admin/{name}", post(admin))
            .with_state(state);

        let response = app
            .oneshot(
                Request::post("/admin/target_user")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"state":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status().is_success(),
            "Expected success but got {}",
            response.status()
        );

        // Verify cache was invalidated - important because is_admin permission changed
        assert!(cache.get("user_token").await.is_none());
    }

    #[tokio::test]
    async fn test_admin_self_demotion_prevented() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("admin".to_string(), true))); // Must be admin
        // Note: change_admin_state should NOT be called because self-demotion is blocked

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/admin/{name}", post(admin))
            .with_state(state);

        let response = app
            .oneshot(
                Request::post("/admin/admin") // Trying to demote self
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"state":false}"#)) // Demoting (state=false)
                    .unwrap(),
            )
            .await
            .unwrap();

        // Request should fail with Bad Request
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "Expected BAD_REQUEST but got {}",
            response.status()
        );
    }

    #[tokio::test]
    async fn test_non_admin_cannot_change_admin_status() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("regular_user".to_string(), false))); // NOT admin

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/admin/{name}", post(admin))
            .with_state(state);

        let response = app
            .oneshot(
                Request::post("/admin/target_user")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"state":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Request should fail with Forbidden
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "Expected FORBIDDEN but got {}",
            response.status()
        );
    }

    #[tokio::test]
    async fn test_admin_demotion_works() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("admin".to_string(), true))); // Must be admin
        mock_db
            .expect_change_admin_state()
            .times(1)
            .with(eq("other_admin"), eq(false)) // Demoting other_admin
            .returning(|_, _| Ok(()));

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/admin/{name}", post(admin))
            .with_state(state);

        let response = app
            .oneshot(
                Request::post("/admin/other_admin")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"state":false}"#)) // Demoting
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status().is_success(),
            "Expected success but got {}",
            response.status()
        );
    }

    #[tokio::test]
    async fn test_admin_nonexistent_user_returns_error() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("admin".to_string(), true))); // Must be admin
        mock_db
            .expect_change_admin_state()
            .times(1)
            .with(eq("nonexistent"), eq(true))
            .returning(|_, _| Err(DbError::UserNotFound("nonexistent".to_string())));

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/admin/{name}", post(admin))
            .with_state(state);

        let response = app
            .oneshot(
                Request::post("/admin/nonexistent")
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"state":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Request should fail with Not Found
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Expected NOT_FOUND but got {}",
            response.status()
        );
    }

    #[tokio::test]
    async fn test_admin_self_promotion_allowed() {
        // An admin promoting themselves (state=true) should be allowed
        // Only self-demotion (state=false) is blocked
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .times(1)
            .returning(|_| Ok(("admin".to_string(), true))); // Must be admin
        mock_db
            .expect_change_admin_state()
            .times(1)
            .with(eq("admin"), eq(true)) // Self-promotion (no-op but allowed)
            .returning(|_, _| Ok(()));

        let state = test_state_with_cache(mock_db, cache.clone());
        let app = Router::new()
            .route("/admin/{name}", post(admin))
            .with_state(state);

        let response = app
            .oneshot(
                Request::post("/admin/admin") // Same user
                    .header(
                        header::COOKIE,
                        encode_cookies([(COOKIE_SESSION_ID, "session")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"state":true}"#)) // Promoting (state=true)
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status().is_success(),
            "Expected success but got {}",
            response.status()
        );
    }
}
