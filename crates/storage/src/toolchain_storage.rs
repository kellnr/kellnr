use bytes::Bytes;

use crate::cached_crate_storage::DynStorage;
use crate::storage_error::StorageError;

pub struct ToolchainStorage {
    storage: DynStorage,
}

impl ToolchainStorage {
    pub fn new(storage: DynStorage) -> Self {
        Self { storage }
    }

    pub fn storage_path(date: &str, name: &str, version: &str, target: &str) -> String {
        format!("{date}/{name}-{version}-{target}.tar.xz")
    }

    pub async fn put(
        &self,
        date: &str,
        name: &str,
        version: &str,
        target: &str,
        archive_data: Bytes,
    ) -> Result<(String, String), StorageError> {
        let path = Self::storage_path(date, name, version, target);
        let hash = sha256::digest(&archive_data[..]);

        self.storage.put(&path, archive_data).await.map_err(|e| {
            if let StorageError::S3Error(object_store::Error::AlreadyExists { .. }) = e {
                return StorageError::ToolchainArchiveExists {
                    name: name.to_string(),
                    version: version.to_string(),
                    target: target.to_string(),
                };
            }
            StorageError::ToolchainStoreFailed {
                name: name.to_string(),
                version: version.to_string(),
                target: target.to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok((path, hash))
    }

    pub async fn get(&self, path: &str) -> Result<Bytes, StorageError> {
        self.storage.get(path).await.map_err(|e| {
            if matches!(
                &e,
                StorageError::S3Error(object_store::Error::NotFound { .. })
                    | StorageError::FileDoesNotExist(_)
            ) {
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

    pub async fn delete(&self, path: &str) -> Result<(), StorageError> {
        self.storage
            .delete(path)
            .await
            .map_err(|e| StorageError::ToolchainDeleteFailed {
                path: path.to_string(),
                reason: e.to_string(),
            })
    }

    pub async fn exists(&self, path: &str) -> Result<bool, StorageError> {
        self.storage.exists(path).await
    }
}
