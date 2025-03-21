use crate::storage_error::StorageError;
use bytes::Bytes;
use moka::future::Cache;
use object_store::{
    ObjectStore, PutMode,
    aws::{AmazonS3, AmazonS3Builder},
    local::LocalFileSystem,
    path::{self, Path},
};
use std::path::PathBuf;

pub enum Storage {
    S3(S3Storage),
    FS(FSStorage),
}

pub type CrateCache = Cache<PathBuf, Vec<u8>>;

impl Storage {
    pub async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        match self {
            Storage::S3(s3_storage) => s3_storage.get(key).await,
            Storage::FS(fsstorage) => fsstorage.get(key).await,
        }
    }

    pub async fn put(&self, key: &str, object: Option<Bytes>) -> Result<(), StorageError> {
        match self {
            Storage::S3(s3_storage) => s3_storage.put(key, object).await,
            Storage::FS(fsstorage) => fsstorage.put(key, object).await,
        }
    }
}

pub struct FSStorage(LocalFileSystem, Option<CrateCache>);
impl FSStorage {
    pub fn new(path: &str, cache_size: u64) -> Result<Self, StorageError> {
        let client = LocalFileSystem::new_with_prefix(path)?;
        let cache = if cache_size > 0 {
            Some(Cache::new(cache_size))
        } else {
            None
        };

        Ok(Self(client, cache))
    }

    fn cache(&self) -> Option<&CrateCache> {
        self.1.as_ref()
    }

    fn storage(&self) -> &LocalFileSystem {
        &self.0
    }

    async fn with_cache(&self, key: &str) -> Result<Bytes, StorageError> {
        let path = PathBuf::from(key);

        async fn fallback(
            storage: &LocalFileSystem,
            key: &str,
        ) -> Result<Bytes, object_store::Error> {
            storage.get(&Path::from(key)).await?.bytes().await
        }

        match self.cache() {
            Some(cache) => {
                if let Some(data) = cache.get(&path).await {
                    Ok(data.into())
                } else {
                    let data = fallback(self.storage(), key).await?;
                    Ok(data)
                }
            }
            None => fallback(self.storage(), key)
                .await
                .map_err(StorageError::from),
        }
    }

    async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        self.with_cache(key).await
    }

    async fn put(&self, key: &str, object: Option<Bytes>) -> Result<(), StorageError> {
        let path = PathBuf::from(key);

        match object {
            Some(object) => {
                self.storage()
                    .put_opts(
                        &Path::from(key),
                        object.clone().into(),
                        PutMode::Create.into(),
                    )
                    .await?;

                if let Some(cache) = self.cache() {
                    cache.insert(path, object.to_vec()).await;
                }
                Ok(())
            }
            None => {
                self.storage().delete(&Path::from(key)).await?;
                self.invalidate_path(&path).await;

                Ok(())
            }
        }
    }

    pub async fn invalidate_path(&self, file_path: &PathBuf) {
        if let Some(cache) = self.cache() {
            cache.invalidate(file_path).await;
        }
    }

    pub fn cache_has_path(&self, file_path: &PathBuf) -> bool {
        self.cache().is_some_and(|f| f.contains_key(file_path))
    }
}

impl From<FSStorage> for Storage {
    fn from(value: FSStorage) -> Self {
        Self::FS(value)
    }
}

pub struct S3Storage(AmazonS3);

impl S3Storage {
    pub fn new(
        region: &str,
        url: &str,
        bucket_name: &str,
        access_key_id: &str,
        secret_access_key: &str,
        allow_http: bool,
    ) -> Result<Self, StorageError> {
        let client = AmazonS3Builder::new()
            .with_endpoint(url)
            .with_bucket_name(bucket_name)
            .with_region(region)
            .with_allow_http(allow_http)
            .with_access_key_id(access_key_id)
            .with_secret_access_key(secret_access_key)
            .with_conditional_put(object_store::aws::S3ConditionalPut::ETagMatch) // MinIO suitable
            .build()?;

        Ok(Self(client))
    }

    fn try_path_from(key: &str) -> Result<Path, object_store::path::Error> {
        let mut prepare_path: Vec<&str> = key.split("/").collect();
        prepare_path.reverse();
        if let Some(crate_name) = prepare_path.first() {
            object_store::path::Path::from_url_path(crate_name)
        } else {
            Err(path::Error::InvalidPath { path: key.into() })
        }
    }

    async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        let path = Self::try_path_from(key)?;
        let get_result = self.0.get(&path).await?;
        let res = get_result.bytes().await?;

        Ok(res)
    }

    async fn put(&self, key: &str, object: Option<Bytes>) -> Result<(), StorageError> {
        let path = Self::try_path_from(key)?;

        if let Some(object) = object {
            if let Err(object_store::Error::AlreadyExists { path: _, source: _ }) = self
                .0
                .put_opts(&path, object.clone().into(), PutMode::Create.into())
                .await
            {
                self.0
                    .put_opts(&path, object.into(), PutMode::Overwrite.into())
                    .await?;
                return Ok(());
            }
            return Ok(());
        }
        self.0.delete(&path).await?;
        Ok(())
    }
}

impl From<S3Storage> for Storage {
    fn from(value: S3Storage) -> Self {
        Self::S3(value)
    }
}
