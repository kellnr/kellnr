use bytes::Bytes;

use crate::cached_crate_storage::DynStorage;
use crate::storage_error::StorageError;
use crate::storage_key::StorageKey;

/// Identifies the archive of a toolchain target, e.g. `rust` `1.75.0` for
/// `x86_64-unknown-linux-gnu`, released on `2024-01-15`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolchainArchiveId<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub date: &'a str,
    pub target: &'a str,
}

pub struct ToolchainStorage {
    storage: DynStorage,
}

impl ToolchainStorage {
    pub fn new(storage: DynStorage) -> Self {
        Self { storage }
    }

    /// Stores the archive of a toolchain target and returns its key and hash.
    pub async fn put(
        &self,
        id: &ToolchainArchiveId<'_>,
        archive_data: Bytes,
    ) -> Result<(StorageKey, String), StorageError> {
        let path = StorageKey::toolchain_archive(id);
        let hash = sha256::digest(&archive_data[..]);

        self.storage.put(&path, archive_data).await.map_err(|e| {
            if let StorageError::ObjectStore(object_store::Error::AlreadyExists { .. }) = e {
                return StorageError::ToolchainArchiveExists {
                    name: id.name.to_string(),
                    version: id.version.to_string(),
                    target: id.target.to_string(),
                };
            }
            StorageError::ToolchainStoreFailed {
                name: id.name.to_string(),
                version: id.version.to_string(),
                target: id.target.to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok((path, hash))
    }

    pub async fn put_raw(&self, path: &StorageKey, data: Bytes) -> Result<(), StorageError> {
        self.storage
            .put(path, data)
            .await
            .map_err(|e| StorageError::ToolchainStoreFailed {
                name: format!("component at {path}"),
                version: String::new(),
                target: String::new(),
                reason: e.to_string(),
            })
    }

    /// Like [`Self::put_raw`], but replaces an existing file.
    pub async fn put_raw_overwrite(
        &self,
        path: &StorageKey,
        data: Bytes,
    ) -> Result<(), StorageError> {
        self.storage.put_overwrite(path, data).await.map_err(|e| {
            StorageError::ToolchainStoreFailed {
                name: format!("component at {path}"),
                version: String::new(),
                target: String::new(),
                reason: e.to_string(),
            }
        })
    }

    pub async fn get(&self, path: &StorageKey) -> Result<Bytes, StorageError> {
        self.storage.get(path).await.map_err(|e| {
            if e.is_not_found() {
                return StorageError::ToolchainNotFound {
                    path: path.to_string(),
                };
            }
            StorageError::ToolchainGetFailed {
                path: path.to_string(),
                reason: e.to_string(),
            }
        })
    }

    pub async fn delete(&self, path: &StorageKey) -> Result<(), StorageError> {
        self.storage
            .delete(path)
            .await
            .map_err(|e| StorageError::ToolchainDeleteFailed {
                path: path.to_string(),
                reason: e.to_string(),
            })
    }

    pub async fn exists(&self, path: &StorageKey) -> Result<bool, StorageError> {
        self.storage.exists(path).await
    }
}
