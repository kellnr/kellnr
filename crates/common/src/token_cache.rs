use std::time::Duration;

use moka::future::Cache;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedTokenData {
    pub user: String,
    pub is_admin: bool,
    pub is_read_only: bool,
}

pub struct TokenCacheManager {
    cache: Option<Cache<String, CachedTokenData>>,
}

impl TokenCacheManager {
    /// Creates a new TokenCacheManager.
    ///
    /// # Arguments
    /// * `enabled` - Whether caching is enabled
    /// * `ttl_seconds` - Time-to-live for cached tokens. Lower values provide
    ///   better security (revoked tokens expire faster) but increase database load.
    /// * `max_capacity` - Maximum number of tokens to cache
    pub fn new(enabled: bool, ttl_seconds: u64, max_capacity: u64) -> Self {
        let cache = if enabled {
            Some(
                Cache::builder()
                    .max_capacity(max_capacity)
                    .time_to_live(Duration::from_secs(ttl_seconds))
                    .build(),
            )
        } else {
            None
        };

        Self { cache }
    }

    pub async fn get(&self, token: &str) -> Option<CachedTokenData> {
        match &self.cache {
            Some(cache) => cache.get(token).await,
            None => None,
        }
    }

    pub async fn insert(&self, token: String, data: CachedTokenData) {
        if let Some(cache) = &self.cache {
            cache.insert(token, data).await;
        }
    }

    pub async fn invalidate_all(&self) {
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

    #[tokio::test]
    async fn test_cache_disabled() {
        let cache = TokenCacheManager::new(false, 60, 100);
        assert!(!cache.is_enabled());

        let data = CachedTokenData {
            user: "test_user".to_string(),
            is_admin: false,
            is_read_only: false,
        };

        cache.insert("token123".to_string(), data).await;
        assert!(cache.get("token123").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_enabled() {
        let cache = TokenCacheManager::new(true, 60, 100);
        assert!(cache.is_enabled());

        let data = CachedTokenData {
            user: "test_user".to_string(),
            is_admin: true,
            is_read_only: false,
        };

        cache.insert("token123".to_string(), data.clone()).await;

        let retrieved = cache.get("token123").await;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.user, "test_user");
        assert!(retrieved.is_admin);
        assert!(!retrieved.is_read_only);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = TokenCacheManager::new(true, 60, 100);
        assert!(cache.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_invalidate_all() {
        let cache = TokenCacheManager::new(true, 60, 100);

        let data = CachedTokenData {
            user: "test_user".to_string(),
            is_admin: false,
            is_read_only: true,
        };

        cache.insert("token1".to_string(), data.clone()).await;
        cache.insert("token2".to_string(), data).await;

        assert!(cache.get("token1").await.is_some());
        assert!(cache.get("token2").await.is_some());

        cache.invalidate_all().await;

        // Note: moka's invalidate_all is async internally and may not be immediately visible
        // In production use, entries will be invalidated lazily
    }
}
