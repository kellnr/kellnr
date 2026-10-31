use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use kellnr_common::account_name::UserName;
use moka::future::Cache;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedTokenData {
    pub user: UserName,
    pub is_admin: bool,
    pub is_read_only: bool,
}

pub struct TokenCacheManager {
    cache: Option<Cache<String, CachedTokenData>>,
    /// Bumped by every [`TokenCacheManager::invalidate_all`]. A lookup that
    /// started before an invalidation must not cache its (possibly stale)
    /// result, so inserts are tied to the generation read before the lookup.
    generation: AtomicU64,
}

impl TokenCacheManager {
    /// Creates a new `TokenCacheManager`.
    ///
    /// # Arguments
    /// * `enabled` - Whether caching is enabled
    /// * `ttl_seconds` - Time-to-live for cached tokens. Lower values provide
    ///   better security (revoked tokens expire faster) but increase database load.
    /// * `max_capacity` - Maximum number of tokens to cache
    pub fn new(enabled: bool, ttl: Duration, max_capacity: u64) -> Self {
        let cache = if enabled {
            Some(
                Cache::builder()
                    .max_capacity(max_capacity)
                    .time_to_live(ttl)
                    .build(),
            )
        } else {
            None
        };

        Self {
            cache,
            generation: AtomicU64::new(0),
        }
    }

    pub async fn get(&self, token: &str) -> Option<CachedTokenData> {
        match &self.cache {
            Some(cache) => cache.get(token).await,
            None => None,
        }
    }

    /// Current invalidation generation. Read it before the database lookup
    /// whose result is passed to [`TokenCacheManager::insert`].
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Caches `data` unless the cache was invalidated since `generation` was
    /// read. The generation is checked again after the insert: an
    /// invalidation racing with the insert removes the fresh entry, so a
    /// revoked token or outdated privileges never outlive an invalidation.
    pub async fn insert(&self, token: String, data: CachedTokenData, generation: u64) {
        let Some(cache) = &self.cache else {
            return;
        };
        if self.generation() != generation {
            return;
        }
        cache.insert(token.clone(), data).await;
        if self.generation() != generation {
            cache.invalidate(&token).await;
        }
    }

    pub fn invalidate_all(&self) {
        // Bump first so in-flight lookups observe the invalidation before the
        // entries are dropped.
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(cache) = &self.cache {
            cache.invalidate_all();
        }
    }

    /// Invalidate a single token from the cache.
    pub async fn invalidate(&self, token: &str) {
        if let Some(cache) = &self.cache {
            cache.invalidate(token).await;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.cache.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(is_admin: bool, is_read_only: bool) -> CachedTokenData {
        CachedTokenData {
            user: UserName::new("test_user").unwrap(),
            is_admin,
            is_read_only,
        }
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let cache = TokenCacheManager::new(false, Duration::from_secs(60), 100);
        assert!(!cache.is_enabled());

        cache
            .insert(
                "token123".to_string(),
                data(false, false),
                cache.generation(),
            )
            .await;
        assert!(cache.get("token123").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_enabled() {
        let cache = TokenCacheManager::new(true, Duration::from_secs(60), 100);
        assert!(cache.is_enabled());

        cache
            .insert(
                "token123".to_string(),
                data(true, false),
                cache.generation(),
            )
            .await;

        let retrieved = cache.get("token123").await;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.user, "test_user");
        assert!(retrieved.is_admin);
        assert!(!retrieved.is_read_only);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = TokenCacheManager::new(true, Duration::from_secs(60), 100);
        assert!(cache.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_invalidate_all() {
        let cache = TokenCacheManager::new(true, Duration::from_secs(60), 100);

        let generation = cache.generation();
        cache
            .insert("token1".to_string(), data(false, true), generation)
            .await;
        cache
            .insert("token2".to_string(), data(false, true), generation)
            .await;

        assert!(cache.get("token1").await.is_some());
        assert!(cache.get("token2").await.is_some());

        cache.invalidate_all();

        assert!(cache.get("token1").await.is_none());
        assert!(cache.get("token2").await.is_none());
    }

    /// A lookup that read the generation before a revocation must not cache
    /// its result afterwards.
    #[tokio::test]
    async fn insert_after_invalidation_is_dropped() {
        let cache = TokenCacheManager::new(true, Duration::from_secs(60), 100);

        let generation = cache.generation();
        cache.invalidate_all();
        cache
            .insert("token".to_string(), data(true, false), generation)
            .await;

        assert!(cache.get("token").await.is_none());
    }

    #[tokio::test]
    async fn invalidate_all_bumps_generation() {
        let cache = TokenCacheManager::new(false, Duration::from_secs(60), 100);
        let before = cache.generation();

        cache.invalidate_all();

        assert_ne!(before, cache.generation());
    }
}
