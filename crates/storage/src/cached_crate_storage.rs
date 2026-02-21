use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_settings::Settings;
use moka::future::Cache;

use crate::storage::Storage;
use crate::storage_error::StorageError;

pub type CrateCache = Cache<String, Bytes>;
pub type DynStorage = Box<dyn Storage + Send + Sync>;

pub struct CachedCrateStorage {
    pub doc_queue_path: PathBuf,
    storage: DynStorage,
    cache: Option<CrateCache>,
}

impl CachedCrateStorage {
    pub fn new(settings: &Settings, storage: DynStorage) -> Self {
        let cache = if settings.registry.cache_size > 0 {
            let max_bytes = settings.registry.cache_size * 1024 * 1024; // cache_size is in MB
            Some(
                Cache::builder()
                    .weigher(|_key: &String, value: &Bytes| -> u32 {
                        u32::try_from(value.len()).unwrap_or(u32::MAX)
                    })
                    .max_capacity(max_bytes)
                    .build(),
            )
        } else {
            None
        };

        Self {
            doc_queue_path: settings.doc_queue_path(),
            storage,
            cache,
        }
    }

    fn file_name(name: &str, version: &str) -> String {
        format!("{name}-{version}.crate")
    }

    pub async fn delete(&self, name: &OriginalName, version: &Version) -> Result<(), StorageError> {
        let crate_file = Self::file_name(name, version);
        self.storage.delete(&crate_file).await?;
        self.invalidate_path(&crate_file).await;
        Ok(())
    }

    pub async fn put(
        &self,
        name: &OriginalName,
        version: &Version,
        crate_data: Arc<[u8]>,
    ) -> Result<String, StorageError> {
        let crate_file = Self::file_name(name, version);
        self.storage
            .put(&crate_file, crate_data.to_vec().into())
            .await
            .map_err(|e| {
                if let StorageError::S3Error(object_store::Error::AlreadyExists {
                    path: _,
                    source: _,
                }) = e
                {
                    return StorageError::CrateExists(name.to_string(), version.to_string());
                }
                StorageError::GenericError(format!("Error while adding bin package. Error: {e}"))
            })?;

        Ok(sha256::digest(&*crate_data))
    }

    pub async fn get(&self, name: &OriginalName, version: &Version) -> Option<Bytes> {
        let file_name = Self::file_name(name, version);
        match self.cache {
            Some(ref cache) => {
                let storage = &self.storage;
                let key = file_name.clone();
                cache
                    .try_get_with(file_name, async move {
                        storage.get(&key).await
                    })
                    .await
                    .ok()
            }
            None => self.storage.get(&file_name).await.ok(),
        }
    }

    async fn invalidate_path(&self, file_path: &str) {
        if let Some(cache) = &self.cache {
            cache.invalidate(file_path).await;
        }
    }

    pub fn cache_has_path(&self, name: &OriginalName, version: &Version) -> bool {
        let file_name = Self::file_name(name, version);
        self.cache
            .as_ref()
            .is_some_and(|cache| cache.contains_key(&file_name))
    }

    // Check if a crate exists in the storage unrelated to the cache.
    pub async fn exists(
        &self,
        name: &OriginalName,
        version: &Version,
    ) -> Result<bool, StorageError> {
        let file_name = Self::file_name(name, version);
        self.storage.exists(&file_name).await
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;

    use super::*;

    /// Shared state for tracking storage call counts.
    struct StorageMetrics {
        get_count: AtomicUsize,
    }

    /// In-memory storage that tracks call counts for observing coalescing.
    struct CountingStorage {
        data: std::collections::HashMap<String, Bytes>,
        metrics: Arc<StorageMetrics>,
    }

    impl CountingStorage {
        fn new(entries: Vec<(&str, &[u8])>, metrics: Arc<StorageMetrics>) -> Self {
            let data = entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), Bytes::from(v.to_vec())))
                .collect();
            Self { data, metrics }
        }
    }

    #[async_trait]
    impl Storage for CountingStorage {
        async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
            self.metrics.get_count.fetch_add(1, Ordering::SeqCst);
            // Simulate slow storage
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            self.data
                .get(key)
                .cloned()
                .ok_or_else(|| StorageError::GenericError(format!("not found: {key}")))
        }

        async fn put(&self, _key: &str, _object: Bytes) -> Result<(), StorageError> {
            Ok(())
        }

        async fn delete(&self, _key: &str) -> Result<(), StorageError> {
            Ok(())
        }

        async fn exists(&self, key: &str) -> Result<bool, StorageError> {
            Ok(self.data.contains_key(key))
        }
    }

    /// Wrapper to make CountingStorage usable through Arc (needed for concurrent test)
    #[async_trait]
    impl Storage for Arc<CountingStorage> {
        async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
            (**self).get(key).await
        }

        async fn put(&self, key: &str, object: Bytes) -> Result<(), StorageError> {
            (**self).put(key, object).await
        }

        async fn delete(&self, key: &str) -> Result<(), StorageError> {
            (**self).delete(key).await
        }

        async fn exists(&self, key: &str) -> Result<bool, StorageError> {
            (**self).exists(key).await
        }
    }

    fn test_settings(cache_size_mb: u64) -> Settings {
        Settings {
            registry: kellnr_settings::Registry {
                data_dir: "/tmp/kellnr-test-cache".to_string(),
                cache_size: cache_size_mb,
                ..kellnr_settings::Registry::default()
            },
            ..Settings::default()
        }
    }

    fn name(s: &str) -> OriginalName {
        OriginalName::try_from(s).unwrap()
    }

    fn ver(s: &str) -> Version {
        Version::try_from(s).unwrap()
    }

    fn metrics() -> Arc<StorageMetrics> {
        Arc::new(StorageMetrics {
            get_count: AtomicUsize::new(0),
        })
    }

    #[tokio::test]
    async fn get_returns_bytes_from_storage() {
        let m = metrics();
        let storage = CountingStorage::new(vec![("mycrate-1.0.0.crate", b"hello")], Arc::clone(&m));
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        let result = cs.get(&name("mycrate"), &ver("1.0.0")).await;
        assert_eq!(result, Some(Bytes::from_static(b"hello")));
    }

    #[tokio::test]
    async fn get_returns_none_for_missing_crate() {
        let m = metrics();
        let storage = CountingStorage::new(vec![], Arc::clone(&m));
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        let result = cs.get(&name("missing"), &ver("1.0.0")).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn get_caches_result_on_second_call() {
        let m = metrics();
        let storage = CountingStorage::new(
            vec![("mycrate-1.0.0.crate", b"cached")],
            Arc::clone(&m),
        );
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        // First call hits storage
        let r1 = cs.get(&name("mycrate"), &ver("1.0.0")).await;
        assert_eq!(r1, Some(Bytes::from_static(b"cached")));

        // Second call should be served from cache
        let r2 = cs.get(&name("mycrate"), &ver("1.0.0")).await;
        assert_eq!(r2, Some(Bytes::from_static(b"cached")));

        // Storage should only have been called once
        assert_eq!(
            m.get_count.load(Ordering::SeqCst),
            1,
            "Storage should only be called once due to caching"
        );
    }

    #[tokio::test]
    async fn concurrent_gets_coalesce_into_single_storage_read() {
        let m = metrics();
        let storage = Arc::new(CountingStorage::new(
            vec![("popular-2.0.0.crate", b"data")],
            Arc::clone(&m),
        ));

        let cs = Arc::new(CachedCrateStorage::new(
            &test_settings(1),
            Box::new(Arc::clone(&storage)),
        ));

        // Launch 50 concurrent requests for the same crate
        let mut join_set = tokio::task::JoinSet::new();
        for _ in 0..50 {
            let cs = Arc::clone(&cs);
            join_set.spawn(async move {
                cs.get(&name("popular"), &ver("2.0.0")).await
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            results.push(result.unwrap());
        }

        // All should succeed with the same data
        for data in &results {
            assert_eq!(data.as_ref(), Some(&Bytes::from_static(b"data")));
        }

        // try_get_with should coalesce: storage called at most a few times
        // (exactly 1 in ideal case, but timing might cause 2-3)
        let count = m.get_count.load(Ordering::SeqCst);
        assert!(
            count <= 3,
            "Expected at most 3 storage reads due to coalescing, got {count}"
        );
    }

    #[tokio::test]
    async fn cache_disabled_when_size_is_zero() {
        let m = metrics();
        let storage = CountingStorage::new(
            vec![("mycrate-1.0.0.crate", b"no-cache")],
            Arc::clone(&m),
        );
        let cs = CachedCrateStorage::new(&test_settings(0), Box::new(storage));

        // Call twice
        let r1 = cs.get(&name("mycrate"), &ver("1.0.0")).await;
        let r2 = cs.get(&name("mycrate"), &ver("1.0.0")).await;
        assert_eq!(r1, Some(Bytes::from_static(b"no-cache")));
        assert_eq!(r2, Some(Bytes::from_static(b"no-cache")));

        // Without cache, storage should be called twice
        assert_eq!(
            m.get_count.load(Ordering::SeqCst),
            2,
            "Without cache, storage should be called each time"
        );
    }

    #[tokio::test]
    async fn failed_storage_reads_are_not_cached() {
        let m = metrics();
        let storage = CountingStorage::new(vec![], Arc::clone(&m));
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        let r1 = cs.get(&name("missing"), &ver("1.0.0")).await;
        assert_eq!(r1, None);

        let r2 = cs.get(&name("missing"), &ver("1.0.0")).await;
        assert_eq!(r2, None);

        // Both calls should have hit storage (errors are NOT cached by try_get_with)
        assert_eq!(
            m.get_count.load(Ordering::SeqCst),
            2,
            "Failed reads should not be cached"
        );
    }

    #[tokio::test]
    async fn cache_has_path_reflects_cached_entries() {
        let m = metrics();
        let storage = CountingStorage::new(
            vec![("mycrate-1.0.0.crate", b"present")],
            Arc::clone(&m),
        );
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        // Before first get, cache doesn't have the entry
        assert!(!cs.cache_has_path(&name("mycrate"), &ver("1.0.0")));

        // After get, it should be cached
        let _ = cs.get(&name("mycrate"), &ver("1.0.0")).await;
        assert!(cs.cache_has_path(&name("mycrate"), &ver("1.0.0")));
    }

    #[tokio::test]
    async fn delete_invalidates_cache() {
        let m = metrics();
        let storage = CountingStorage::new(
            vec![("mycrate-1.0.0.crate", b"to-delete")],
            Arc::clone(&m),
        );
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        // Populate cache
        let _ = cs.get(&name("mycrate"), &ver("1.0.0")).await;
        assert!(cs.cache_has_path(&name("mycrate"), &ver("1.0.0")));

        // Delete should invalidate
        // (will fail on CountingStorage::delete but invalidate_path still runs)
        let _ = cs.delete(&name("mycrate"), &ver("1.0.0")).await;
        assert!(!cs.cache_has_path(&name("mycrate"), &ver("1.0.0")));
    }

    #[tokio::test]
    async fn concurrent_gets_for_missing_crate_all_return_none() {
        let m = metrics();
        let storage = Arc::new(CountingStorage::new(vec![], Arc::clone(&m)));

        let cs = Arc::new(CachedCrateStorage::new(
            &test_settings(1),
            Box::new(Arc::clone(&storage)),
        ));

        // Launch 20 concurrent requests for a missing crate
        let mut join_set = tokio::task::JoinSet::new();
        for _ in 0..20 {
            let cs = Arc::clone(&cs);
            join_set.spawn(async move {
                cs.get(&name("absent"), &ver("1.0.0")).await
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            results.push(result.unwrap());
        }

        // All should return None
        for data in &results {
            assert_eq!(*data, None);
        }
    }

    #[tokio::test]
    async fn concurrent_gets_for_different_crates_are_independent() {
        let m = metrics();
        let storage = Arc::new(CountingStorage::new(
            vec![
                ("crate-a-1.0.0.crate", b"aaa"),
                ("crate-b-1.0.0.crate", b"bbb"),
            ],
            Arc::clone(&m),
        ));

        let cs = Arc::new(CachedCrateStorage::new(
            &test_settings(1),
            Box::new(Arc::clone(&storage)),
        ));

        // Launch concurrent requests for two different crates
        let mut join_set = tokio::task::JoinSet::new();
        for i in 0..20 {
            let cs = Arc::clone(&cs);
            let crate_name = if i % 2 == 0 { "crate-a" } else { "crate-b" };
            join_set.spawn(async move {
                cs.get(&name(crate_name), &ver("1.0.0")).await
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            results.push(result.unwrap());
        }

        // All should return Some
        for data in &results {
            assert!(data.is_some());
        }

        // Should have called storage at most a few times for each key
        // (2 unique keys, ideally 1 call each, possibly more due to timing)
        let count = m.get_count.load(Ordering::SeqCst);
        assert!(
            count <= 6,
            "Expected at most 6 storage reads for 2 unique keys, got {count}"
        );
    }
}
