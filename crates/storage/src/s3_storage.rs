use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kellnr_settings::Settings;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::path::Path;
use object_store::{ClientOptions, ObjectStore, ObjectStoreExt, PutMode};

use crate::storage::Storage;
use crate::storage_error::StorageError;

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
        // NOTE: `with_client_options` replaces the builder's internal ClientOptions entirely,
        // so `allow_http` must be set here rather than via `AmazonS3Builder::with_allow_http`,
        // which would be overwritten by the subsequent `with_client_options` call.
        let client_options = ClientOptions::new()
            .with_connect_timeout(Duration::from_secs(settings.s3.connect_timeout_seconds))
            .with_timeout(Duration::from_secs(settings.s3.request_timeout_seconds))
            .with_allow_http(settings.s3.allow_http);

        let mut s3 = AmazonS3Builder::from_env()
            .with_bucket_name(bucket)
            .with_client_options(client_options)
            .with_conditional_put(object_store::aws::S3ConditionalPut::ETagMatch);
        if let Some(endpoint) = &settings.s3.endpoint {
            s3 = s3.with_endpoint(endpoint);
        }
        if let Some(region) = &settings.s3.region {
            s3 = s3.with_region(region);
        }
        if let Some(access_key) = &settings.s3.access_key {
            s3 = s3.with_access_key_id(access_key);
        }
        if let Some(secret_key) = &settings.s3.secret_key {
            s3 = s3.with_secret_access_key(secret_key);
        }
        // S3-compatible (RustFS, MinIO, etc.)
        Ok(Self(s3.build()?))
    }
}
