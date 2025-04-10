use crate::{storage::Storage, storage_error::StorageError};
use common::original_name::OriginalName;
use common::version::Version;
use moka::future::Cache;
use settings::Settings;
use std::{path::PathBuf, sync::Arc};

pub type CrateCache = Cache<String, Vec<u8>>;
pub type DynStorage = Box<dyn Storage + Send + Sync>;

pub struct CachedCrateStorage {
    pub doc_queue_path: PathBuf,
    storage: DynStorage,
    cache: Option<CrateCache>,
}

impl CachedCrateStorage {
    pub fn new(settings: &Settings, storage: DynStorage) -> Result<Self, StorageError> {
        let cache = if settings.registry.cache_size > 0 {
            Some(Cache::new(settings.registry.cache_size))
        } else {
            None
        };

        let cs = Self {
            doc_queue_path: settings.doc_queue_path(),
            storage,
            cache,
        };
        Ok(cs)
    }

    fn file_name(name: &str, version: &str) -> String {
        format!("{}-{}.crate", name, version)
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
                StorageError::GenericError(format!("Error while adding bin package. Error: {}", e))
            })?;

        Ok(sha256::digest(&*crate_data))
    }

    pub async fn get(&self, name: &OriginalName, version: &Version) -> Option<Vec<u8>> {
        let file_name = Self::file_name(name, version);
        match self.cache {
            Some(ref cache) => {
                if let Some(data) = cache.get(&file_name).await {
                    Some(data.to_vec())
                } else {
                    let data = self.storage.get(&file_name).await.ok()?;
                    cache.insert(file_name.to_owned(), data.to_vec()).await;
                    Some(data.to_vec())
                }
            }
            None => self.storage.get(&file_name).await.map(<Vec<u8>>::from).ok(),
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
            .map(|cache| cache.contains_key(&file_name))
            .unwrap_or(false)
    }
}
