use std::fmt;

use bytes::Bytes;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;

use crate::cached_crate_storage::DynStorage;
use crate::storage::ObjectWithMeta;
use crate::storage_error::StorageError;
use crate::storage_key::StorageKey;

/// Name of the folder that holds the docs of a crate: the name the crate was
/// published with, in lowercase. Docs links point to it, so it must not change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocsName(String);

impl DocsName {
    /// A folder of docs that was stored before, kept as it is.
    pub fn from_folder(name: String) -> Self {
        Self(name)
    }
}

impl From<&OriginalName> for DocsName {
    fn from(name: &OriginalName) -> Self {
        Self(name.to_lowercase())
    }
}

impl fmt::Display for DocsName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct DocsStorage {
    storage: DynStorage,
}

impl DocsStorage {
    pub fn new(storage: DynStorage) -> Self {
        Self { storage }
    }

    /// Key for one file under a crate+version's doc tree, e.g.
    /// `my-crate/1.0.0/doc/my_crate/index.html`.
    pub fn file_key(crate_name: &DocsName, version: &Version, relative_path: &str) -> StorageKey {
        StorageKey::docs_file(crate_name, version, relative_path)
    }

    pub fn version_prefix(crate_name: &DocsName, version: &Version) -> String {
        format!("{crate_name}/{version}/")
    }

    pub async fn put(&self, key: &StorageKey, data: Bytes) -> Result<(), StorageError> {
        self.storage
            .put_overwrite(key, data)
            .await
            .map_err(|e| StorageError::DocsStoreFailed {
                key: key.to_string(),
                reason: e.to_string(),
            })
    }

    pub async fn get(&self, key: &StorageKey) -> Result<Bytes, StorageError> {
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

    pub async fn get_with_meta(&self, key: &StorageKey) -> Result<ObjectWithMeta, StorageError> {
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

    pub async fn exists(&self, key: &StorageKey) -> Result<bool, StorageError> {
        self.storage.exists(key).await
    }

    pub async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        self.storage
            .delete(key)
            .await
            .map_err(|e| StorageError::DocsDeleteFailed {
                key: key.to_string(),
                reason: e.to_string(),
            })
    }

    pub async fn list(&self, prefix: &str) -> Result<Vec<StorageKey>, StorageError> {
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
