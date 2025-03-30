use crate::{storage::Storage, storage_error::StorageError};
use common::original_name::OriginalName;
use common::version::Version;
use moka::future::Cache;
use settings::Settings;
use std::{path::PathBuf, sync::Arc};

pub type CrateCache = Cache<String, Vec<u8>>;
pub type DynStorage = Box<dyn Storage + Send + Sync>;

pub struct CachedCrateStorage {
    crate_folder: String,
    pub doc_queue_path: PathBuf,
    storage: DynStorage,
    cache: Option<CrateCache>,
}

impl CachedCrateStorage {
    pub fn new(
        crate_folder: &str,
        settings: &Settings,
        storage: DynStorage,
    ) -> Result<Self, StorageError> {
        let cache = if settings.registry.cache_size > 0 {
            Some(Cache::new(settings.registry.cache_size))
        } else {
            None
        };

        let cs = Self {
            crate_folder: crate_folder.to_owned(),
            doc_queue_path: settings.doc_queue_path(),
            storage,
            cache,
        };
        Ok(cs)
    }

    pub fn crate_path(&self, name: &str, version: &str) -> String {
        format!("{}/{}-{}.crate", &self.crate_folder, name, version)
    }

    pub async fn delete(&self, name: &OriginalName, version: &Version) -> Result<(), StorageError> {
        let crate_path = self.crate_path(name, version);
        self.storage.delete(&crate_path).await?;
        self.invalidate_path(&crate_path).await;
        Ok(())
    }

    pub async fn put(
        &self,
        name: &OriginalName,
        version: &Version,
        crate_data: Arc<[u8]>,
    ) -> Result<String, StorageError> {
        let crate_path = self.crate_path(name, version);
        self.storage
            .put(&crate_path, crate_data.to_vec().into())
            .await
            .map_err(|e| {
                if let StorageError::S3Error(object_store::Error::AlreadyExists {
                    path: _,
                    source: _,
                }) = e
                {
                    return StorageError::CrateExists(name.to_string(), version.to_string());
                }
                StorageError::GenericError(format!("Error while adding bin package. Error: {}", e))
            })?;

        Ok(sha256::digest(&*crate_data))
    }

    pub async fn get(&self, file_path: &str) -> Option<Vec<u8>> {
        match self.cache {
            Some(ref cache) => {
                if let Some(data) = cache.get(file_path).await {
                    Some(data.to_vec())
                } else {
                    let data = self.storage.get(file_path).await.ok()?;
                    cache.insert(file_path.to_owned(), data.to_vec()).await;
                    Some(data.to_vec())
                }
            }
            None => self.storage.get(file_path).await.map(<Vec<u8>>::from).ok(),
        }
    }

    async fn invalidate_path(&self, file_path: &str) {
        if let Some(cache) = &self.cache {
            cache.invalidate(file_path).await;
        }
    }

    pub fn cache_has_path(&self, file_path: &str) -> bool {
        self.cache
            .as_ref()
            .map(|cache| cache.contains_key(file_path))
            .unwrap_or(false)
    }
}
