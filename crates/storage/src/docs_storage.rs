use std::collections::HashSet;

use bytes::Bytes;

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
    /// `file_key("my-crate", "1.0.0", "doc/my_crate/index.html")`.
    pub fn file_key(crate_name: &str, version: &str, relative_path: &str) -> String {
        format!("{crate_name}/{version}/{relative_path}")
    }

    pub fn version_prefix(crate_name: &str, version: &str) -> String {
        format!("{crate_name}/{version}/")
    }

    pub fn crate_prefix(crate_name: &str) -> String {
        format!("{crate_name}/")
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
            if matches!(
                &e,
                StorageError::S3Error(object_store::Error::NotFound { .. })
                    | StorageError::FileDoesNotExist(_)
            ) {
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
            if matches!(
                &e,
                StorageError::S3Error(object_store::Error::NotFound { .. })
                    | StorageError::FileDoesNotExist(_)
            ) {
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

    /// Distinct version-folder names that have *any* key under `crate_name/`.
    /// Callers still need to verify a given version actually has an
    /// `index.html` — this only enumerates candidates.
    pub async fn version_candidates(&self, crate_name: &str) -> Result<Vec<String>, StorageError> {
        let prefix = Self::crate_prefix(crate_name);
        let mut versions = HashSet::new();
        for key in self.list(&prefix).await? {
            if let Some(rest) = key.strip_prefix(&prefix)
                && let Some(v) = rest.split('/').next()
            {
                versions.insert(v.to_string());
            }
        }
        Ok(versions.into_iter().collect())
    }
}
