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
