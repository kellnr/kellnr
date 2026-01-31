use std::iter;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use kellnr_appstate::AppStateData;
use kellnr_common::token_cache::{CachedTokenData, TokenCacheManager};
use kellnr_db::DbProvider;
use kellnr_db::error::DbError;
use kellnr_settings::Settings;
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use serde::Deserialize;
use tokio::time::sleep;
use tracing::warn;
use utoipa::ToSchema;

#[derive(Debug)]
pub struct Token {
    pub value: String,
    pub user: String,
    pub is_admin: bool,
    pub is_read_only: bool,
}

// See https://github.com/tokio-rs/axum/discussions/2281
#[derive(Debug)]
pub enum OptionToken {
    None,
    Some(Token),
}

pub fn generate_token() -> String {
    let mut rng = rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(32)
        .collect::<String>()
}

impl Token {
    pub async fn from_header(
        headers: &HeaderMap,
        db: &Arc<dyn DbProvider>,
        cache: &Arc<TokenCacheManager>,
        settings: &Arc<Settings>,
    ) -> Result<Self, StatusCode> {
        Self::extract_token(headers, db, cache, settings).await
    }

    async fn extract_token(
        headers: &HeaderMap,
        db: &Arc<dyn DbProvider>,
        cache: &Arc<TokenCacheManager>,
        settings: &Arc<Settings>,
    ) -> Result<Token, StatusCode> {
        // OptionToken code expects UNAUTHORIZED when no token is found
        let mut token = headers
            .get("Authorization")
            .ok_or(StatusCode::UNAUTHORIZED)?
            .to_str()
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        // Handle basic authentication (does NOT use token cache - queries DB directly)
        if token.starts_with("Basic ") || token.starts_with("basic ") {
            let decoded = STANDARD
                .decode(&token[6..])
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            let decoded_str = String::from_utf8(decoded).map_err(|_| StatusCode::BAD_REQUEST)?;
            let (user, token) = decoded_str.split_once(':').ok_or(StatusCode::BAD_REQUEST)?;

            let user = db.get_user(user).await.map_err(|_| StatusCode::FORBIDDEN)?;
            if db.authenticate_user(&user.name, token).await.is_err() {
                return Err(StatusCode::FORBIDDEN);
            }

            return Ok(Token {
                value: token.to_string(),
                user: user.name,
                is_admin: user.is_admin,
                is_read_only: user.is_read_only,
            });
        }

        // Handle bearer authentication (uses token cache)
        if token.starts_with("Bearer ") || token.starts_with("bearer ") {
            token = &token[7..];
        }

        // Check cache first
        if let Some(cached) = cache.get(token).await {
            return Ok(Token {
                value: token.to_string(),
                user: cached.user,
                is_admin: cached.is_admin,
                is_read_only: cached.is_read_only,
            });
        }

        // Cache miss - query DB with retry logic
        let Ok(user) = get_user_with_retry(
            db,
            token,
            settings.registry.token_db_retry_count,
            settings.registry.token_db_retry_delay_ms,
        )
        .await
        else {
            return Err(StatusCode::FORBIDDEN);
        };

        // Insert into cache on successful DB lookup
        cache
            .insert(
                token.to_string(),
                CachedTokenData {
                    user: user.name.clone(),
                    is_admin: user.is_admin,
                    is_read_only: user.is_read_only,
                },
            )
            .await;

        Ok(Token {
            value: token.to_string(),
            user: user.name,
            is_admin: user.is_admin,
            is_read_only: user.is_read_only,
        })
    }
}

async fn get_user_with_retry(
    db: &Arc<dyn DbProvider>,
    token: &str,
    max_retries: u32,
    delay_ms: u64,
) -> Result<kellnr_db::User, DbError> {
    let mut attempts = 0;

    loop {
        match db.get_user_from_token(token).await {
            Ok(user) => return Ok(user),
            Err(e) => {
                // Do not retry on "not found" errors - these are definitive
                if matches!(e, DbError::TokenNotFound | DbError::UserNotFound(_)) {
                    return Err(e);
                }

                attempts += 1;
                if attempts > max_retries {
                    warn!(
                        "Failed to get user from token after {} retries: {}",
                        max_retries, e
                    );
                    return Err(e);
                }

                warn!(
                    "Transient DB error on attempt {}/{}, retrying in {}ms: {}",
                    attempts,
                    max_retries + 1,
                    delay_ms,
                    e
                );
                sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
}

impl FromRequestParts<AppStateData> for Token {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        Self::extract_token(
            &parts.headers,
            &state.db,
            &state.token_cache,
            &state.settings,
        )
        .await
    }
}

impl FromRequestParts<AppStateData> for OptionToken {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        match Token::extract_token(
            &parts.headers,
            &state.db,
            &state.token_cache,
            &state.settings,
        )
        .await
        {
            Ok(token) => Ok(OptionToken::Some(token)),
            Err(StatusCode::UNAUTHORIZED) => Ok(OptionToken::None),
            Err(status_code) => Err(status_code),
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct NewTokenReqData {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use kellnr_db::User;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use mockall::predicate::*;

    use super::*;

    fn test_user() -> User {
        User {
            id: 1,
            name: "test_user".to_string(),
            pwd: String::new(),
            salt: String::new(),
            is_admin: false,
            is_read_only: false,
            created: String::new(),
        }
    }

    fn test_settings() -> Arc<Settings> {
        Arc::new(Settings {
            registry: kellnr_settings::Registry {
                token_cache_enabled: true,
                token_cache_ttl_seconds: 60,
                token_cache_max_capacity: 100,
                token_db_retry_count: 3,
                token_db_retry_delay_ms: 1,
                ..kellnr_settings::Registry::default()
            },
            ..Settings::default()
        })
    }

    // ===================
    // Retry Logic Tests
    // ===================

    #[tokio::test]
    async fn test_retry_succeeds_on_first_attempt() {
        // DB returns user on first try - no retries needed
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("valid_token"))
            .times(1)
            .returning(|_| Ok(test_user()));

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let result = get_user_with_retry(&db, "valid_token", 3, 10).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "test_user");
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_transient_error() {
        // DB fails once with transient error, then succeeds
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("token"))
            .times(2)
            .returning(move |_| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    Err(DbError::PostgresError(sea_orm::DbErr::ConnectionAcquire(
                        sea_orm::error::ConnAcquireErr::Timeout,
                    )))
                } else {
                    Ok(test_user())
                }
            });

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let result = get_user_with_retry(&db, "token", 3, 1).await;

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_no_retry_on_token_not_found() {
        // TokenNotFound should NOT trigger retries
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("invalid_token"))
            .times(1) // Should only be called once
            .returning(|_| Err(DbError::TokenNotFound));

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let result = get_user_with_retry(&db, "invalid_token", 3, 10).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DbError::TokenNotFound));
    }

    #[tokio::test]
    async fn test_no_retry_on_user_not_found() {
        // UserNotFound should NOT trigger retries
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("orphan_token"))
            .times(1)
            .returning(|_| Err(DbError::UserNotFound("orphan".to_string())));

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let result = get_user_with_retry(&db, "orphan_token", 3, 10).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DbError::UserNotFound(_)));
    }

    #[tokio::test]
    async fn test_exhausts_retries_on_persistent_error() {
        // DB fails all retries with transient error
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("token"))
            .times(4) // 1 initial + 3 retries
            .returning(move |_| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Err(DbError::PostgresError(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::error::ConnAcquireErr::Timeout,
                )))
            });

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let result = get_user_with_retry(&db, "token", 3, 1).await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
    }

    // =====================================
    // Token Extraction with Cache Tests
    // =====================================

    #[tokio::test]
    async fn test_cache_hit_returns_cached_token() {
        // Pre-populate cache, DB should NOT be called
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));
        cache
            .insert(
                "cached_token".to_string(),
                CachedTokenData {
                    user: "cached_user".to_string(),
                    is_admin: true,
                    is_read_only: false,
                },
            )
            .await;

        let mock_db = MockDb::new(); // No expectations - should not be called
        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let settings = test_settings();

        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer cached_token".parse().unwrap());

        let result = Token::from_header(&headers, &db, &cache, &settings).await;

        assert!(result.is_ok());
        let token = result.unwrap();
        assert_eq!(token.user, "cached_user");
        assert!(token.is_admin);
    }

    #[tokio::test]
    async fn test_cache_miss_queries_db_and_caches() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("new_token"))
            .times(1)
            .returning(|_| {
                Ok(User {
                    id: 1,
                    name: "db_user".to_string(),
                    pwd: String::new(),
                    salt: String::new(),
                    is_admin: false,
                    is_read_only: true,
                    created: String::new(),
                })
            });

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let settings = test_settings();

        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer new_token".parse().unwrap());

        let result = Token::from_header(&headers, &db, &cache, &settings).await;

        assert!(result.is_ok());
        let token = result.unwrap();
        assert_eq!(token.user, "db_user");
        assert!(token.is_read_only);

        // Verify token was cached
        let cached = cache.get("new_token").await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().user, "db_user");
    }

    #[tokio::test]
    async fn test_cache_miss_with_invalid_token_returns_forbidden() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("bad_token"))
            .times(1) // Should not retry on TokenNotFound
            .returning(|_| Err(DbError::TokenNotFound));

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let settings = test_settings();

        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer bad_token".parse().unwrap());

        let result = Token::from_header(&headers, &db, &cache, &settings).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);

        // Verify invalid token was NOT cached
        let cached = cache.get("bad_token").await;
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_no_authorization_header_returns_unauthorized() {
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));
        let mock_db = MockDb::new();
        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let settings = test_settings();

        let headers = HeaderMap::new(); // No Authorization header

        let result = Token::from_header(&headers, &db, &cache, &settings).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_disabled_cache_always_queries_db() {
        // Cache is disabled - should always query DB
        let cache = Arc::new(TokenCacheManager::new(false, 60, 100)); // Disabled

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("token"))
            .times(2) // Called twice
            .returning(move |_| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(test_user())
            });

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let settings = test_settings();

        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer token".parse().unwrap());

        // First call
        let _ = Token::from_header(&headers, &db, &cache, &settings).await;
        // Second call - should hit DB again since cache is disabled
        let _ = Token::from_header(&headers, &db, &cache, &settings).await;

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_lowercase_bearer_prefix_works() {
        // Verifies case-insensitive handling of "bearer" prefix
        let cache = Arc::new(TokenCacheManager::new(true, 60, 100));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("lowercase_token"))
            .times(1)
            .returning(|_| Ok(test_user()));

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let settings = test_settings();

        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "bearer lowercase_token".parse().unwrap());

        let result = Token::from_header(&headers, &db, &cache, &settings).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().user, "test_user");
    }

    #[tokio::test]
    async fn test_zero_retries_only_attempts_once() {
        // With max_retries = 0, should only attempt once
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq("token"))
            .times(1)
            .returning(move |_| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Err(DbError::PostgresError(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::error::ConnAcquireErr::Timeout,
                )))
            });

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let result = get_user_with_retry(&db, "token", 0, 1).await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
