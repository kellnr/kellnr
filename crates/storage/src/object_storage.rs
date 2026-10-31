use async_trait::async_trait;
use bytes::Bytes;
use futures_util::StreamExt;
use object_store::memory::InMemory;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutMode};
use percent_encoding::percent_decode_str;

use crate::storage::{ObjectWithMeta, Storage};
use crate::storage_error::StorageError;
use crate::storage_key::StorageKey;

/// [`Storage`] on top of an `object_store` backend.
pub struct ObjectStorage<S>(pub(crate) S);

/// Storage that keeps all objects in memory, e.g. for tests. Unlike the file
/// system on some platforms, its keys are case-sensitive.
pub type MemoryStorage = ObjectStorage<InMemory>;

impl MemoryStorage {
    pub fn new() -> Self {
        Self(InMemory::new())
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// The object path of `key`, the same for all backends. Characters that are
/// not allowed in object keys are percent-encoded.
fn object_path(key: &str) -> Path {
    Path::from(key)
}

/// The key of an object path, the inverse of [`object_path`]. Keys returned
/// by [`Storage::list`] are therefore equal to the keys they were stored with.
fn object_key(path: &Path) -> StorageKey {
    let key = path
        .parts()
        .map(|part| {
            percent_decode_str(part.as_ref())
                .decode_utf8_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>()
        .join("/");
    StorageKey::from_stored(key)
}

#[async_trait]
impl<S: ObjectStore> Storage for ObjectStorage<S> {
    async fn get(&self, key: &StorageKey) -> Result<Bytes, StorageError> {
        Ok(self
            .0
            .get(&object_path(key.as_str()))
            .await?
            .bytes()
            .await?)
    }

    async fn get_with_meta(&self, key: &StorageKey) -> Result<ObjectWithMeta, StorageError> {
        let result = self.0.get(&object_path(key.as_str())).await?;
        let e_tag = result.meta.e_tag.clone();
        let last_modified = result.meta.last_modified;
        let bytes = result.bytes().await?;
        Ok(ObjectWithMeta {
            bytes,
            e_tag,
            last_modified,
        })
    }

    async fn put(&self, key: &StorageKey, object: Bytes) -> Result<(), StorageError> {
        self.0
            .put_opts(
                &object_path(key.as_str()),
                object.into(),
                PutMode::Create.into(),
            )
            .await?;
        Ok(())
    }

    async fn put_overwrite(&self, key: &StorageKey, object: Bytes) -> Result<(), StorageError> {
        self.0
            .put_opts(
                &object_path(key.as_str()),
                object.into(),
                PutMode::Overwrite.into(),
            )
            .await?;
        Ok(())
    }

    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        self.0.delete(&object_path(key.as_str())).await?;
        Ok(())
    }

    async fn exists(&self, key: &StorageKey) -> Result<bool, StorageError> {
        match self.0.head(&object_path(key.as_str())).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    async fn size(&self, key: &StorageKey) -> Result<u64, StorageError> {
        Ok(self.0.head(&object_path(key.as_str())).await?.size)
    }

    async fn list(&self, prefix: &str) -> Result<Vec<StorageKey>, StorageError> {
        let prefix = (!prefix.is_empty()).then(|| object_path(prefix));
        let mut stream = self.0.list(prefix.as_ref());
        let mut keys = Vec::new();
        while let Some(meta) = stream.next().await {
            keys.push(object_key(&meta?.location));
        }
        Ok(keys)
    }
}
