use crate::{storage::Storage, storage_error::StorageError};
use async_trait::async_trait;
use bytes::Bytes;
use object_store::{
    ObjectStore, PutMode,
    aws::{AmazonS3, AmazonS3Builder},
    path::{self, Path},
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
}

impl S3Storage {
    pub fn new(
        region: &str,
        url: &str,
        access_key_id: &str,
        secret_access_key: &str,
        allow_http: bool,
        bucket: &str,
    ) -> Result<Self, StorageError> {
        let client = AmazonS3Builder::new()
            .with_endpoint(url)
            .with_bucket_name(bucket)
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

    fn storage(&self) -> &AmazonS3 {
        &self.0
    }
}

impl TryFrom<(&str, &Settings)> for S3Storage {
    type Error = StorageError;

    fn try_from((bucket, settings): (&str, &Settings)) -> Result<Self, Self::Error> {
        S3Storage::new(
            &settings.s3.region,
            &settings.s3.endpoint,
            &settings.s3.access_key,
            &settings.s3.secret_key,
            settings.s3.allow_http,
            bucket,
        )
    }
}
