use std::fs::DirBuilder;

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::StreamExt;
use object_store::local::LocalFileSystem;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutMode};

use crate::storage::{ObjectWithMeta, Storage};
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

    async fn get_with_meta(&self, key: &str) -> Result<ObjectWithMeta, StorageError> {
        let result = self.storage().get(&Path::from(key)).await?;
        let e_tag = result.meta.e_tag.clone();
        let last_modified = result.meta.last_modified;
        let bytes = result.bytes().await?;
        Ok(ObjectWithMeta {
            bytes,
            e_tag,
            last_modified,
        })
    }

    async fn put(&self, key: &str, object: Bytes) -> Result<(), StorageError> {
        self.storage()
            .put_opts(&Path::from(key), object.into(), PutMode::Create.into())
            .await?;
        Ok(())
    }

    async fn put_overwrite(&self, key: &str, object: Bytes) -> Result<(), StorageError> {
        self.storage()
            .put_opts(&Path::from(key), object.into(), PutMode::Overwrite.into())
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

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        let prefix = if prefix.is_empty() {
            None
        } else {
            Some(Path::from(prefix))
        };
        let mut stream = self.storage().list(prefix.as_ref());
        let mut keys = Vec::new();
        while let Some(meta) = stream.next().await {
            keys.push(meta?.location.to_string());
        }
        Ok(keys)
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
