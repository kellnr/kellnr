use bytes::Bytes;
use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::version::Version;

use crate::cached_crate_storage::DynStorage;
use crate::storage::ObjectWithMeta;
use crate::storage_error::StorageError;

pub struct DocsStorage {
    storage: DynStorage,
}

impl DocsStorage {
    pub fn new(storage: DynStorage) -> Self {
        Self { storage }
    }

    /// Key for one file under a crate+version's doc tree, e.g.
    /// `my-crate/1.0.0/doc/my_crate/index.html`.
    pub fn file_key(crate_name: &NormalizedName, version: &Version, relative_path: &str) -> String {
        format!("{crate_name}/{version}/{relative_path}")
    }

    pub fn version_prefix(crate_name: &NormalizedName, version: &Version) -> String {
        format!("{crate_name}/{version}/")
    }

    pub async fn put(&self, key: &str, data: Bytes) -> Result<(), StorageError> {
        self.storage
            .put_overwrite(key, data)
            .await
            .map_err(|e| StorageError::DocsStoreFailed {
                key: key.to_string(),
                reason: e.to_string(),
            })
    }

    pub async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        self.storage.get(key).await.map_err(|e| {
            if e.is_not_found() {
                return StorageError::DocsNotFound {
                    key: key.to_string(),
                };
            }
            StorageError::DocsGetFailed {
                key: key.to_string(),
                reason: e.to_string(),
            }
        })
    }

    pub async fn get_with_meta(&self, key: &str) -> Result<ObjectWithMeta, StorageError> {
        self.storage.get_with_meta(key).await.map_err(|e| {
            if e.is_not_found() {
                return StorageError::DocsNotFound {
                    key: key.to_string(),
                };
            }
            StorageError::DocsGetFailed {
                key: key.to_string(),
                reason: e.to_string(),
            }
        })
    }

    pub async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        self.storage.exists(key).await
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.storage
            .delete(key)
            .await
            .map_err(|e| StorageError::DocsDeleteFailed {
                key: key.to_string(),
                reason: e.to_string(),
            })
    }

    pub async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        self.storage
            .list(prefix)
            .await
            .map_err(|e| StorageError::DocsListFailed {
                prefix: prefix.to_string(),
                reason: e.to_string(),
            })
    }

    /// Delete every key under `prefix` (used for crate/version teardown).
    pub async fn delete_prefix(&self, prefix: &str) -> Result<(), StorageError> {
        for key in self.list(prefix).await? {
            self.delete(&key).await?;
        }
        Ok(())
    }
}
