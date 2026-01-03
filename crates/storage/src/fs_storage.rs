use std::fs::DirBuilder;

use async_trait::async_trait;
use bytes::Bytes;
use object_store::local::LocalFileSystem;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutMode};

use crate::storage::Storage;
use crate::storage_error::StorageError;

pub struct FSStorage(LocalFileSystem);

#[async_trait]
impl Storage for FSStorage {
    async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        self.storage()
            .get(&Path::from(key))
            .await?
            .bytes()
            .await
            .map_err(StorageError::from)
    }

    async fn put(&self, key: &str, object: Bytes) -> Result<(), StorageError> {
        self.storage()
            .put_opts(&Path::from(key), object.into(), PutMode::Create.into())
            .await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.storage().delete(&Path::from(key)).await?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        self.storage()
            .head(&Path::from(key))
            .await
            .map(|_| true)
            .or_else(|e| match e {
                object_store::Error::NotFound { .. } => Ok(false),
                _ => Err(StorageError::from(e)),
            })
    }
}

impl FSStorage {
    pub fn new(crate_folder: &str) -> Result<Self, StorageError> {
        let path = std::path::Path::new(crate_folder);
        if !path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(crate_folder)
                .map_err(|e| StorageError::CreateBinPath(path.to_path_buf(), e))?;
        }
        let client = LocalFileSystem::new_with_prefix(path)?;
        Ok(Self(client))
    }

    fn storage(&self) -> &LocalFileSystem {
        &self.0
    }
}
