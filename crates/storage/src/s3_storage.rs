use std::time::Duration;

use kellnr_settings::Settings;
use object_store::ClientOptions;
use object_store::aws::{AmazonS3, AmazonS3Builder};

use crate::object_storage::ObjectStorage;
use crate::storage_error::StorageError;

pub type S3Storage = ObjectStorage<AmazonS3>;

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
