use std::iter;
use std::sync::Arc;
use std::time::Duration;

use axum::http::{HeaderMap, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use kellnr_common::account_name::UserName;
use kellnr_common::plain_token::PlainToken;
use kellnr_db::DbProvider;
use kellnr_db::error::DbError;
use kellnr_db::password::hash_token;
use rand::distr::Alphanumeric;
use rand::{RngExt, rng};
use serde::Deserialize;
use tokio::time::sleep;
use tracing::warn;
use utoipa::ToSchema;

use crate::password::{PasswordAuthError, authenticate_password};
use crate::state::AppStateData;
use crate::token_cache::CachedTokenData;

/// The user of the cargo token or basic auth credentials of a request.
#[derive(Debug)]
pub(crate) struct Token {
    /// The auth token, `None` for basic auth with a password
    pub value: Option<PlainToken>,
    pub user: UserName,
    pub is_admin: bool,
    pub is_read_only: bool,
}

pub fn generate_token() -> PlainToken {
    let mut rng = rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(32)
        .collect::<String>()
        .into()
}

impl Token {
    /// The user of the `Authorization` header. Without the header, the
    /// result is 401.
    pub(crate) async fn from_header(
        headers: &HeaderMap,
        state: &AppStateData,
    ) -> Result<Token, StatusCode> {
        let (db, cache) = (&state.db, &state.token_cache);
        let mut token = headers
            .get("Authorization")
            .ok_or(StatusCode::UNAUTHORIZED)?
            .to_str()
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        // Handle basic authentication
        if let Some(credentials) = strip_scheme(token, "Basic") {
            if state.settings.oauth2.enforced {
                return Err(StatusCode::FORBIDDEN);
            }

            let decoded = STANDARD
                .decode(credentials)
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            let decoded_str = String::from_utf8(decoded).map_err(|_| StatusCode::BAD_REQUEST)?;
            let (user, pwd) = decoded_str.split_once(':').ok_or(StatusCode::BAD_REQUEST)?;

            return authenticate_basic(user, pwd, state).await;
        }

        // Handle bearer authentication (uses token cache)
        if let Some(bearer) = strip_scheme(token, "Bearer") {
            token = bearer;
        }

        // Check cache first
        if let Some(cached) = cache.get(token).await {
            return Ok(Token {
                value: Some(PlainToken::from(token)),
                user: cached.user,
                is_admin: cached.is_admin,
                is_read_only: cached.is_read_only,
            });
        }

        // Cache miss - query DB with retry logic. The generation is read
        // before the lookup so a concurrent invalidation prevents caching a
        // result that may already be outdated.
        let generation = cache.generation();
        let Ok(user) = get_user_with_retry(
            db,
            &PlainToken::from(token),
            state.settings.registry.token_db_retry_count,
            state.settings.registry.token_db_retry_delay_ms,
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
                generation,
            )
            .await;

        Ok(Token {
            value: Some(PlainToken::from(token)),
            user: user.name,
            is_admin: user.is_admin,
            is_read_only: user.is_read_only,
        })
    }
}

/// Cache key for verified basic auth credentials. Only a hash of the
/// credentials is kept in memory, and the prefix keeps the key space apart
/// from bearer tokens.
fn basic_auth_cache_key(user: &str, pwd: &str) -> String {
    format!("basic:{}", hash_token(&format!("{user}:{pwd}")))
}

/// Verifies basic auth credentials. Cargo sends them with every request and
/// password hashing is deliberately slow, so successful verifications are
/// cached like bearer tokens and dropped by the same invalidations. A cache
/// hit implies correct credentials, so it bypasses the attempt throttle and
/// keeps CI working while an attacker locks out the username.
async fn authenticate_basic(
    user: &str,
    pwd: &str,
    state: &AppStateData,
) -> Result<Token, StatusCode> {
    let cache = &state.token_cache;
    let cache_key = basic_auth_cache_key(user, pwd);
    if let Some(cached) = cache.get(&cache_key).await {
        return Ok(Token {
            value: None,
            user: cached.user,
            is_admin: cached.is_admin,
            is_read_only: cached.is_read_only,
        });
    }

    // An empty user name cannot exist, so it fails like a wrong password.
    let user = UserName::new(user).map_err(|_| StatusCode::FORBIDDEN)?;
    let generation = cache.generation();
    let user = authenticate_password(state.db.as_ref(), &state.login_throttle, &user, pwd)
        .await
        .map_err(|e| match e {
            PasswordAuthError::Throttled => StatusCode::TOO_MANY_REQUESTS,
            PasswordAuthError::Invalid => StatusCode::FORBIDDEN,
        })?;

    cache
        .insert(
            cache_key,
            CachedTokenData {
                user: user.name.clone(),
                is_admin: user.is_admin,
                is_read_only: user.is_read_only,
            },
            generation,
        )
        .await;

    Ok(Token {
        value: None,
        user: user.name,
        is_admin: user.is_admin,
        is_read_only: user.is_read_only,
    })
}

async fn get_user_with_retry(
    db: &Arc<dyn DbProvider>,
    token: &PlainToken,
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

#[derive(Deserialize, ToSchema)]
pub struct NewTokenReqData {
    pub name: String,
}

/// The credentials of an `Authorization` header value with the given auth
/// `scheme`, which is compared case-insensitively as required by RFC 9110.
fn strip_scheme<'a>(value: &'a str, scheme: &str) -> Option<&'a str> {
    let (prefix, rest) = value.split_at_checked(scheme.len())?;
    let credentials = rest.strip_prefix(' ')?;
    prefix.eq_ignore_ascii_case(scheme).then_some(credentials)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{User, UserFlags};
    use kellnr_settings::Settings;
    use kellnr_test_db::TestDb;
    use mockall::predicate::*;

    use super::*;
    use crate::login_throttle::LoginThrottle;
    use crate::test_support::sqlite_test_state;
    use crate::token_cache::TokenCacheManager;

    fn test_user() -> User {
        User {
            id: kellnr_db::UserId::new(1),
            name: UserName::new("test_user").unwrap(),
            pwd: String::new(),
            salt: String::new(),
            is_admin: false,
            is_read_only: false,
            created: String::new(),
        }
    }

    fn test_user_name() -> UserName {
        UserName::new("test_user").unwrap()
    }

    fn test_settings() -> Arc<Settings> {
        let mut settings = kellnr_settings::test_settings();
        settings.registry.token_cache_enabled = true;
        settings.registry.token_cache_ttl_seconds = 60;
        settings.registry.token_cache_max_capacity = 100;
        settings.registry.token_db_retry_count = 3;
        settings.registry.token_db_retry_delay_ms = 1;
        Arc::new(settings)
    }

    fn cache() -> Arc<TokenCacheManager> {
        Arc::new(TokenCacheManager::new(true, Duration::from_secs(60), 100))
    }

    /// App state on a database with the user `test_user`, its password
    /// `correct_password` and its cargo token `valid_token`.
    async fn db_state(
        cache: &Arc<TokenCacheManager>,
        settings: Arc<Settings>,
    ) -> (AppStateData, TestDb) {
        let (state, db) = sqlite_test_state().await;
        db.add_user(&test_user_name(), "correct_password", UserFlags::default())
            .await
            .unwrap();
        db.add_auth_token("test", &PlainToken::from("valid_token"), &test_user_name())
            .await
            .unwrap();
        let state = AppStateData {
            token_cache: cache.clone(),
            settings,
            ..state
        };
        (state, db)
    }

    /// Settings with SSO-only login enforced.
    fn sso_enforced_settings() -> Arc<Settings> {
        let mut settings = Settings::clone(&test_settings());
        settings.oauth2.enabled = true;
        settings.oauth2.enforced = true;
        Arc::new(settings)
    }

    fn basic_auth_header(user: &str, pwd: &str) -> HeaderMap {
        let encoded = STANDARD.encode(format!("{user}:{pwd}"));
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", format!("Basic {encoded}").parse().unwrap());
        headers
    }

    fn bearer_header(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", format!("Bearer {token}").parse().unwrap());
        headers
    }

    async fn change_pwd(db: &TestDb, pwd: &str) {
        db.change_pwd(&test_user_name(), pwd).await.unwrap();
    }

    // ===================
    // Retry Logic Tests
    // ===================

    #[tokio::test]
    async fn test_retry_succeeds_on_first_attempt() {
        let (state, _db) = db_state(&cache(), test_settings()).await;

        let result = get_user_with_retry(&state.db, &PlainToken::from("valid_token"), 3, 10).await;

        assert_eq!(result.unwrap().name, "test_user");
    }

    // The tests below inject transient database errors and count the
    // attempts, so they need a mock database.

    #[tokio::test]
    async fn test_retry_succeeds_after_transient_error() {
        // DB fails once with transient error, then succeeds
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq(PlainToken::from("token")))
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
        let result = get_user_with_retry(&db, &PlainToken::from("token"), 3, 1).await;

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    /// An unknown token is final, so the lookup must not wait for a retry.
    #[tokio::test]
    async fn test_no_retry_on_token_not_found() {
        let (state, _db) = db_state(&cache(), test_settings()).await;

        let result = tokio::time::timeout(
            Duration::from_secs(10),
            get_user_with_retry(&state.db, &PlainToken::from("invalid_token"), 3, 60_000),
        )
        .await
        .expect("lookup was retried");

        assert!(matches!(result.unwrap_err(), DbError::TokenNotFound));
    }

    #[tokio::test]
    async fn test_no_retry_on_user_not_found() {
        // UserNotFound should NOT trigger retries
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq(PlainToken::from("orphan_token")))
            .times(1)
            .returning(|_| Err(DbError::UserNotFound("orphan".to_string())));

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let result = get_user_with_retry(&db, &PlainToken::from("orphan_token"), 3, 10).await;

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
            .with(eq(PlainToken::from("token")))
            .times(4) // 1 initial + 3 retries
            .returning(move |_| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Err(DbError::PostgresError(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::error::ConnAcquireErr::Timeout,
                )))
            });

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let result = get_user_with_retry(&db, &PlainToken::from("token"), 3, 1).await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_zero_retries_only_attempts_once() {
        // With max_retries = 0, should only attempt once
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq(PlainToken::from("token")))
            .times(1)
            .returning(move |_| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Err(DbError::PostgresError(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::error::ConnAcquireErr::Timeout,
                )))
            });

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let result = get_user_with_retry(&db, &PlainToken::from("token"), 0, 1).await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    // =====================================
    // Token Extraction with Cache Tests
    // =====================================

    /// The cached token does not exist in the database, so only the cache
    /// can know it.
    #[tokio::test]
    async fn test_cache_hit_returns_cached_token() {
        let cache = cache();
        cache
            .insert(
                "cached_token".to_string(),
                CachedTokenData {
                    user: UserName::new("cached_user").unwrap(),
                    is_admin: true,
                    is_read_only: false,
                },
                cache.generation(),
            )
            .await;
        let (state, _db) = db_state(&cache, test_settings()).await;

        let token = Token::from_header(&bearer_header("cached_token"), &state)
            .await
            .unwrap();

        assert_eq!(token.user, "cached_user");
        assert!(token.is_admin);
    }

    #[tokio::test]
    async fn test_cache_miss_queries_db_and_caches() {
        let cache = cache();
        let (state, db) = db_state(&cache, test_settings()).await;
        let db_user = UserName::new("db_user").unwrap();
        let flags = UserFlags {
            read_only: true,
            ..UserFlags::default()
        };
        db.add_user(&db_user, "pwd", flags).await.unwrap();
        db.add_auth_token("test", &PlainToken::from("new_token"), &db_user)
            .await
            .unwrap();

        let token = Token::from_header(&bearer_header("new_token"), &state)
            .await
            .unwrap();

        assert_eq!(token.user, "db_user");
        assert!(token.is_read_only);
        let cached = cache.get("new_token").await;
        assert_eq!(cached.unwrap().user, "db_user");
    }

    /// A token revoked while its DB lookup is in flight must not end up in
    /// the cache, otherwise it would stay usable for the whole TTL. The mock
    /// revokes it in the middle of the lookup.
    #[tokio::test]
    async fn test_invalidation_during_lookup_is_not_cached() {
        let cache = cache();
        let cache_in_db = cache.clone();

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_user_from_token()
            .with(eq(PlainToken::from("racy_token")))
            .times(1)
            .returning(move |_| {
                cache_in_db.invalidate_all();
                Ok(test_user())
            });

        let state = AppStateData {
            db: Arc::new(mock_db),
            token_cache: cache.clone(),
            settings: test_settings(),
            ..crate::test_support::test_state()
        };

        let token = Token::from_header(&bearer_header("racy_token"), &state)
            .await
            .unwrap();

        assert_eq!(token.user, "test_user");
        assert!(cache.get("racy_token").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_miss_with_invalid_token_returns_forbidden() {
        let cache = cache();
        let (state, _db) = db_state(&cache, test_settings()).await;

        let result = Token::from_header(&bearer_header("bad_token"), &state).await;

        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
        assert!(cache.get("bad_token").await.is_none());
    }

    // ==========================
    // SSO Enforcement Tests
    // ==========================

    /// Basic auth carries a username and password, so it must be rejected
    /// outright when SSO is the only permitted login method, even with the
    /// correct password.
    #[tokio::test]
    async fn test_basic_auth_rejected_when_sso_enforced() {
        let (state, _db) = db_state(&cache(), sso_enforced_settings()).await;
        let headers = basic_auth_header("test_user", "correct_password");

        let result = Token::from_header(&headers, &state).await;

        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
    }

    /// The lowercase spelling of the scheme must not slip past the check.
    #[tokio::test]
    async fn test_lowercase_basic_auth_rejected_when_sso_enforced() {
        let (state, _db) = db_state(&cache(), sso_enforced_settings()).await;
        let encoded = STANDARD.encode("test_user:correct_password");
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", format!("basic {encoded}").parse().unwrap());

        let result = Token::from_header(&headers, &state).await;

        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
    }

    /// Enforcement only targets passwords. Cargo tokens are independent of the
    /// login method and must keep working so publishing does not break.
    #[tokio::test]
    async fn test_bearer_token_still_works_when_sso_enforced() {
        let (state, _db) = db_state(&cache(), sso_enforced_settings()).await;

        let token = Token::from_header(&bearer_header("valid_token"), &state)
            .await
            .expect("bearer tokens must remain valid under SSO enforcement");

        assert_eq!(token.user, "test_user");
    }

    /// Without enforcement, basic auth keeps authenticating against the DB.
    #[tokio::test]
    async fn test_basic_auth_allowed_when_sso_not_enforced() {
        let (state, _db) = db_state(&cache(), test_settings()).await;
        let headers = basic_auth_header("test_user", "correct_password");

        let token = Token::from_header(&headers, &state)
            .await
            .expect("basic auth should work when SSO is not enforced");

        assert_eq!(token.user, "test_user");
    }

    // ==========================
    // Basic Auth Cache Tests
    // ==========================

    /// Password hashing is slow by design, so cargo requests with the same
    /// credentials must not verify the password again. The password changes
    /// in the database after the first request, so the later requests can
    /// only succeed from the cache.
    #[tokio::test]
    async fn test_basic_auth_success_is_cached() {
        let (state, db) = db_state(&cache(), test_settings()).await;
        let headers = basic_auth_header("test_user", "correct_password");

        Token::from_header(&headers, &state).await.unwrap();
        change_pwd(&db, "new_password").await;

        for _ in 0..2 {
            let token = Token::from_header(&headers, &state).await.unwrap();
            assert_eq!(token.user, "test_user");
        }
    }

    /// A failed login is checked again, so it succeeds once the password is
    /// valid.
    #[tokio::test]
    async fn test_basic_auth_failure_is_not_cached() {
        let (state, db) = db_state(&cache(), test_settings()).await;
        let headers = basic_auth_header("test_user", "new_password");

        let result = Token::from_header(&headers, &state).await;
        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
        change_pwd(&db, "new_password").await;

        let token = Token::from_header(&headers, &state).await.unwrap();
        assert_eq!(token.user, "test_user");
    }

    /// A different password for a cached user must still be verified.
    #[tokio::test]
    async fn test_basic_auth_cache_is_keyed_by_password() {
        let (state, _db) = db_state(&cache(), test_settings()).await;

        let ok = basic_auth_header("test_user", "correct_password");
        Token::from_header(&ok, &state).await.unwrap();

        let wrong = basic_auth_header("test_user", "wrong_password");
        let result = Token::from_header(&wrong, &state).await;
        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
    }

    /// Password changes invalidate the whole cache, after which the old
    /// credentials must be verified against the DB again.
    #[tokio::test]
    async fn test_basic_auth_cache_respects_invalidation() {
        let cache = cache();
        let (state, db) = db_state(&cache, test_settings()).await;
        let headers = basic_auth_header("test_user", "correct_password");

        Token::from_header(&headers, &state).await.unwrap();
        change_pwd(&db, "new_password").await;
        cache.invalidate_all();

        let result = Token::from_header(&headers, &state).await;
        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
    }

    /// Brute forcing a password through cargo basic auth is throttled per
    /// username, and the password is no longer checked once locked.
    #[tokio::test]
    async fn test_basic_auth_failures_are_throttled() {
        let (state, _db) = db_state(&cache(), test_settings()).await;
        let state = AppStateData {
            login_throttle: Arc::new(LoginThrottle::new(2, Duration::from_mins(5))),
            ..state
        };
        let guess = basic_auth_header("test_user", "guess");

        for _ in 0..2 {
            let result = Token::from_header(&guess, &state).await;
            assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
        }
        let valid = basic_auth_header("test_user", "correct_password");
        let result = Token::from_header(&valid, &state).await;
        assert_eq!(result.unwrap_err(), StatusCode::TOO_MANY_REQUESTS);
    }

    /// Credentials verified before the lockout keep working from the cache,
    /// so an attacker cannot lock out CI that uses basic auth.
    #[tokio::test]
    async fn test_cached_basic_auth_bypasses_throttle() {
        let (state, _db) = db_state(&cache(), test_settings()).await;
        let state = AppStateData {
            login_throttle: Arc::new(LoginThrottle::new(1, Duration::from_mins(5))),
            ..state
        };
        let valid = basic_auth_header("test_user", "correct_password");
        Token::from_header(&valid, &state).await.unwrap();

        let guess = basic_auth_header("test_user", "guess");
        assert_eq!(
            Token::from_header(&guess, &state).await.unwrap_err(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            Token::from_header(&guess, &state).await.unwrap_err(),
            StatusCode::TOO_MANY_REQUESTS
        );

        let token = Token::from_header(&valid, &state).await.unwrap();
        assert_eq!(token.user, "test_user");
    }

    #[test]
    fn test_basic_auth_cache_key_does_not_contain_password() {
        let key = basic_auth_cache_key("test_user", "secret");

        assert!(key.starts_with("basic:"));
        assert!(!key.contains("secret"));
        assert_ne!(key, basic_auth_cache_key("test_user", "other"));
    }

    #[tokio::test]
    async fn test_no_authorization_header_returns_unauthorized() {
        let state = crate::test_support::test_state();

        let result = Token::from_header(&HeaderMap::new(), &state).await;

        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    /// Without the cache, a revoked token stops working at once.
    #[tokio::test]
    async fn test_disabled_cache_always_queries_db() {
        let cache = Arc::new(TokenCacheManager::new(false, Duration::from_secs(60), 100));
        let (state, db) = db_state(&cache, test_settings()).await;
        let headers = bearer_header("valid_token");

        Token::from_header(&headers, &state).await.unwrap();
        let tokens = db.get_auth_tokens(&test_user_name()).await.unwrap();
        db.delete_auth_token(&test_user_name(), tokens[0].id)
            .await
            .unwrap();

        let result = Token::from_header(&headers, &state).await;
        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_lowercase_bearer_prefix_works() {
        let (state, _db) = db_state(&cache(), test_settings()).await;
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "bearer valid_token".parse().unwrap());

        let token = Token::from_header(&headers, &state).await.unwrap();

        assert_eq!(token.user, "test_user");
    }

    #[test]
    fn auth_scheme_is_case_insensitive() {
        for value in ["Bearer abc", "bearer abc", "BEARER abc", "bEaReR abc"] {
            assert_eq!(strip_scheme(value, "Bearer"), Some("abc"), "{value}");
        }
        assert_eq!(strip_scheme("BASIC abc", "Basic"), Some("abc"));
        assert_eq!(strip_scheme("Bearerabc", "Bearer"), None);
        assert_eq!(strip_scheme("abc", "Bearer"), None);
        assert_eq!(strip_scheme("Bäarer abc", "Bearer"), None);
    }
}
