use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_settings::{Registry, Settings};
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
        Self {
            doc_queue_path: settings.doc_queue_path(),
            storage,
            cache: build_cache(&settings.registry),
        }
    }

    fn file_name(name: &OriginalName, version: &Version) -> String {
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
        crate_data: Bytes,
    ) -> Result<String, StorageError> {
        let crate_file = Self::file_name(name, version);
        self.storage
            .put(&crate_file, crate_data.clone())
            .await
            .map_err(|e| {
                if let StorageError::ObjectStore(object_store::Error::AlreadyExists {
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

    /// The crate file, or `None` if it does not exist. Other storage errors,
    /// e.g. timeouts of a remote storage, are returned as errors.
    pub async fn get(
        &self,
        name: &OriginalName,
        version: &Version,
    ) -> Result<Option<Bytes>, StorageError> {
        let file_name = Self::file_name(name, version);
        let result = match self.cache {
            Some(ref cache) => {
                let storage = &self.storage;
                let key = file_name.clone();
                cache
                    .try_get_with(file_name, async move { storage.get(&key).await })
                    .await
                    .map_err(|e| Arc::try_unwrap(e).unwrap_or_else(StorageError::Shared))
            }
            None => self.storage.get(&file_name).await,
        };
        match result {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.is_not_found() => Ok(None),
            Err(e) => Err(e),
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

    /// Size of a crate file in bytes, read from the storage and not the cache.
    pub async fn size(&self, name: &OriginalName, version: &Version) -> Result<u64, StorageError> {
        let file_name = Self::file_name(name, version);
        self.storage.size(&file_name).await
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

/// A cache for at most `cache_size` crates and, if `cache_max_size` is set,
/// at most `cache_max_size` MB.
fn build_cache(registry: &Registry) -> Option<CrateCache> {
    if registry.cache_size == 0 {
        return None;
    }
    if registry.cache_max_size == 0 {
        return Some(Cache::builder().max_capacity(registry.cache_size).build());
    }
    let max_bytes = registry.cache_max_size.saturating_mul(1_000_000);
    // Every crate weighs at least its share of the size limit, so the cache
    // also never holds more than `cache_size` crates.
    let min_weight = max_bytes.div_ceil(registry.cache_size);
    Some(
        Cache::builder()
            .max_capacity(max_bytes)
            .weigher(move |_key, value: &Bytes| {
                let weight = (value.len() as u64).max(min_weight);
                u32::try_from(weight).unwrap_or(u32::MAX)
            })
            .build(),
    )
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;

    use super::*;
    use crate::storage::ObjectWithMeta;

    /// Shared state for tracking storage call counts.
    struct StorageMetrics {
        get_count: AtomicUsize,
        /// When true, `storage.get()` returns immediately. When false, it waits.
        ready: std::sync::atomic::AtomicBool,
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
            // Wait until the test signals readiness
            while !self.metrics.ready.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
            if key.starts_with("broken") {
                return Err(StorageError::GenericError(
                    "storage unavailable".to_string(),
                ));
            }
            self.data
                .get(key)
                .cloned()
                .ok_or_else(|| StorageError::ObjectStore(not_found(key)))
        }

        async fn get_with_meta(&self, key: &str) -> Result<ObjectWithMeta, StorageError> {
            let bytes = self.get(key).await?;
            Ok(ObjectWithMeta {
                bytes,
                e_tag: None,
                last_modified: chrono::Utc::now(),
            })
        }

        async fn put(&self, _key: &str, _object: Bytes) -> Result<(), StorageError> {
            Ok(())
        }

        async fn put_overwrite(&self, _key: &str, _object: Bytes) -> Result<(), StorageError> {
            Ok(())
        }

        async fn delete(&self, _key: &str) -> Result<(), StorageError> {
            Ok(())
        }

        async fn exists(&self, key: &str) -> Result<bool, StorageError> {
            Ok(self.data.contains_key(key))
        }

        async fn size(&self, key: &str) -> Result<u64, StorageError> {
            self.data
                .get(key)
                .map(|v| v.len() as u64)
                .ok_or_else(|| StorageError::GenericError(format!("not found: {key}")))
        }

        async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
            Ok(self
                .data
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect())
        }
    }

    fn not_found(key: &str) -> object_store::Error {
        object_store::Error::NotFound {
            path: key.to_string(),
            source: "not found".into(),
        }
    }

    /// Wrapper to make `CountingStorage` usable through `Arc` (needed for concurrent test)
    #[async_trait]
    impl Storage for Arc<CountingStorage> {
        async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
            (**self).get(key).await
        }

        async fn get_with_meta(&self, key: &str) -> Result<ObjectWithMeta, StorageError> {
            (**self).get_with_meta(key).await
        }

        async fn put(&self, key: &str, object: Bytes) -> Result<(), StorageError> {
            (**self).put(key, object).await
        }

        async fn put_overwrite(&self, key: &str, object: Bytes) -> Result<(), StorageError> {
            (**self).put_overwrite(key, object).await
        }

        async fn delete(&self, key: &str) -> Result<(), StorageError> {
            (**self).delete(key).await
        }

        async fn exists(&self, key: &str) -> Result<bool, StorageError> {
            (**self).exists(key).await
        }

        async fn size(&self, key: &str) -> Result<u64, StorageError> {
            (**self).size(key).await
        }

        async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
            (**self).list(prefix).await
        }
    }

    fn test_settings(cache_size: u64) -> Settings {
        Settings {
            registry: Registry {
                data_dir: "/tmp/kellnr-test-cache".to_string(),
                cache_size,
                ..Registry::default()
            },
            ..Settings::default()
        }
    }

    fn test_registry(cache_size: u64, cache_max_size: u64) -> Registry {
        Registry {
            cache_size,
            cache_max_size,
            ..Registry::default()
        }
    }

    async fn cached_weight(cache: &CrateCache, entries: &[(&str, usize)]) -> (u64, u64) {
        for (key, size) in entries {
            cache
                .insert((*key).to_string(), Bytes::from(vec![0; *size]))
                .await;
        }
        cache.run_pending_tasks().await;
        let bytes = cache.iter().map(|(_, v)| v.len() as u64).sum();
        (cache.entry_count(), bytes)
    }

    #[tokio::test]
    async fn cache_without_max_size_limits_the_number_of_crates() {
        let cache = build_cache(&test_registry(2, 0)).unwrap();

        let (count, _) = cached_weight(&cache, &[("a", 10), ("b", 10), ("c", 10)]).await;

        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn cache_with_max_size_limits_the_total_size() {
        let cache = build_cache(&test_registry(100, 1)).unwrap();

        let (count, bytes) =
            cached_weight(&cache, &[("a", 400_000), ("b", 400_000), ("c", 400_000)]).await;

        assert_eq!(count, 2);
        assert!(bytes <= 1_000_000, "{bytes}");
    }

    #[tokio::test]
    async fn cache_with_max_size_still_limits_the_number_of_crates() {
        let cache = build_cache(&test_registry(2, 1)).unwrap();

        let (count, _) = cached_weight(&cache, &[("a", 10), ("b", 10), ("c", 10)]).await;

        assert_eq!(count, 2);
    }

    #[test]
    fn cache_is_disabled_with_size_zero() {
        assert!(build_cache(&test_registry(0, 1)).is_none());
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
            ready: std::sync::atomic::AtomicBool::new(true),
        })
    }

    #[tokio::test]
    async fn get_returns_bytes_from_storage() {
        let m = metrics();
        let storage = CountingStorage::new(vec![("mycrate-1.0.0.crate", b"hello")], Arc::clone(&m));
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        let result = cs.get(&name("mycrate"), &ver("1.0.0")).await.unwrap();
        assert_eq!(result, Some(Bytes::from_static(b"hello")));
    }

    #[tokio::test]
    async fn get_returns_none_for_missing_crate() {
        let m = metrics();
        let storage = CountingStorage::new(vec![], Arc::clone(&m));
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        let result = cs.get(&name("missing"), &ver("1.0.0")).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn get_caches_result_on_second_call() {
        let m = metrics();
        let storage =
            CountingStorage::new(vec![("mycrate-1.0.0.crate", b"cached")], Arc::clone(&m));
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        // First call hits storage
        let r1 = cs.get(&name("mycrate"), &ver("1.0.0")).await.unwrap();
        assert_eq!(r1, Some(Bytes::from_static(b"cached")));

        // Second call should be served from cache
        let r2 = cs.get(&name("mycrate"), &ver("1.0.0")).await.unwrap();
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
        let m = Arc::new(StorageMetrics {
            get_count: AtomicUsize::new(0),
            ready: std::sync::atomic::AtomicBool::new(false),
        });
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
            join_set.spawn(async move { cs.get(&name("popular"), &ver("2.0.0")).await.unwrap() });
        }

        // Let all tasks start and queue up, then release them
        tokio::task::yield_now().await;
        m.ready.store(true, Ordering::SeqCst);

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
        let storage =
            CountingStorage::new(vec![("mycrate-1.0.0.crate", b"no-cache")], Arc::clone(&m));
        let cs = CachedCrateStorage::new(&test_settings(0), Box::new(storage));

        // Call twice
        let r1 = cs.get(&name("mycrate"), &ver("1.0.0")).await.unwrap();
        let r2 = cs.get(&name("mycrate"), &ver("1.0.0")).await.unwrap();
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

        let r1 = cs.get(&name("missing"), &ver("1.0.0")).await.unwrap();
        assert_eq!(r1, None);

        let r2 = cs.get(&name("missing"), &ver("1.0.0")).await.unwrap();
        assert_eq!(r2, None);

        // Both calls should have hit storage (errors are NOT cached by try_get_with)
        assert_eq!(
            m.get_count.load(Ordering::SeqCst),
            2,
            "Failed reads should not be cached"
        );
    }

    #[tokio::test]
    async fn get_returns_storage_errors_other_than_not_found() {
        for cache_size in [0, 1] {
            let m = metrics();
            let storage = CountingStorage::new(vec![], Arc::clone(&m));
            let cs = CachedCrateStorage::new(&test_settings(cache_size), Box::new(storage));

            assert!(cs.get(&name("broken"), &ver("1.0.0")).await.is_err());
            assert!(cs.get(&name("broken"), &ver("1.0.0")).await.is_err());

            assert_eq!(m.get_count.load(Ordering::SeqCst), 2, "{cache_size}");
        }
    }

    #[tokio::test]
    async fn cache_has_path_reflects_cached_entries() {
        let m = metrics();
        let storage =
            CountingStorage::new(vec![("mycrate-1.0.0.crate", b"present")], Arc::clone(&m));
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        // Before first get, cache doesn't have the entry
        assert!(!cs.cache_has_path(&name("mycrate"), &ver("1.0.0")));

        // After get, it should be cached
        let _ = cs.get(&name("mycrate"), &ver("1.0.0")).await.unwrap();
        assert!(cs.cache_has_path(&name("mycrate"), &ver("1.0.0")));
    }

    #[tokio::test]
    async fn delete_invalidates_cache() {
        let m = metrics();
        let storage =
            CountingStorage::new(vec![("mycrate-1.0.0.crate", b"to-delete")], Arc::clone(&m));
        let cs = CachedCrateStorage::new(&test_settings(1), Box::new(storage));

        // Populate cache
        let _ = cs.get(&name("mycrate"), &ver("1.0.0")).await.unwrap();
        assert!(cs.cache_has_path(&name("mycrate"), &ver("1.0.0")));

        // Delete should invalidate
        // (will fail on CountingStorage::delete but invalidate_path still runs)
        let _ = cs.delete(&name("mycrate"), &ver("1.0.0")).await;
        assert!(!cs.cache_has_path(&name("mycrate"), &ver("1.0.0")));
    }

    #[tokio::test]
    async fn concurrent_gets_for_missing_crate_all_return_none() {
        let m = Arc::new(StorageMetrics {
            get_count: AtomicUsize::new(0),
            ready: std::sync::atomic::AtomicBool::new(false),
        });
        let storage = Arc::new(CountingStorage::new(vec![], Arc::clone(&m)));

        let cs = Arc::new(CachedCrateStorage::new(
            &test_settings(1),
            Box::new(Arc::clone(&storage)),
        ));

        // Launch 20 concurrent requests for a missing crate
        let mut join_set = tokio::task::JoinSet::new();
        for _ in 0..20 {
            let cs = Arc::clone(&cs);
            join_set.spawn(async move { cs.get(&name("absent"), &ver("1.0.0")).await.unwrap() });
        }

        // Let all tasks start and queue up, then release them
        tokio::task::yield_now().await;
        m.ready.store(true, Ordering::SeqCst);

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
        let m = Arc::new(StorageMetrics {
            get_count: AtomicUsize::new(0),
            ready: std::sync::atomic::AtomicBool::new(false),
        });
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
            join_set.spawn(async move { cs.get(&name(crate_name), &ver("1.0.0")).await.unwrap() });
        }

        // Let all tasks start and queue up, then release them
        tokio::task::yield_now().await;
        m.ready.store(true, Ordering::SeqCst);

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
