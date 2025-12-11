use crate::{storage::Storage, storage_error::StorageError};
use async_trait::async_trait;
use bytes::Bytes;
use object_store::{
    ObjectStore, PutMode,
    aws::{AmazonS3, AmazonS3Builder},
    path::Path,
};
use settings::Settings;

pub struct S3Storage(AmazonS3);

#[async_trait]
impl Storage for S3Storage {
    async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        self.storage()
            .get(&Self::try_path_from(key)?)
            .await?
            .bytes()
            .await
            .map_err(StorageError::from)
    }

    async fn put(&self, key: &str, object: Bytes) -> Result<(), StorageError> {
        self.storage()
            .put_opts(
                &Self::try_path_from(key)?,
                object.into(),
                PutMode::Create.into(),
            )
            .await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = Self::try_path_from(key)?;
        self.storage().delete(&path).await?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        let path = Self::try_path_from(key)?;
        self.storage()
            .head(&path)
            .await
            .map(|_| true)
            .or_else(|e| match e {
                object_store::Error::NotFound { .. } => Ok(false),
                _ => Err(StorageError::from(e)),
            })
    }
}

impl S3Storage {
    fn try_path_from(key: &str) -> Result<Path, object_store::path::Error> {
        Path::from_url_path(key)
    }

    fn storage(&self) -> &AmazonS3 {
        &self.0
    }
}

impl TryFrom<(&str, &Settings)> for S3Storage {
    type Error = StorageError;

    fn try_from((bucket, settings): (&str, &Settings)) -> Result<Self, Self::Error> {
        let mut s3 = AmazonS3Builder::from_env()
            .with_bucket_name(bucket)
            .with_allow_http(settings.s3.allow_http)
            .with_conditional_put(object_store::aws::S3ConditionalPut::ETagMatch);
        if let Some(value) = &settings.s3.endpoint {
            s3 = s3.with_endpoint(value);
        }
        if let Some(value) = &settings.s3.region {
            s3 = s3.with_region(value);
        }
        if let Some(value) = &settings.s3.access_key {
            s3 = s3.with_access_key_id(value);
        }
        if let Some(value) = &settings.s3.secret_key {
            s3 = s3.with_secret_access_key(value);
        }
        // MinIO suitable
        Ok(Self(s3.build()?))
    }
}
