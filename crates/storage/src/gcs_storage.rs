use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::StreamExt;
use kellnr_settings::Settings;
use object_store::gcp::{GoogleCloudStorage, GoogleCloudStorageBuilder};
use object_store::path::Path;
use object_store::{ClientOptions, ObjectStore, ObjectStoreExt, PutMode};

use crate::storage::{ObjectWithMeta, Storage};
use crate::storage_error::StorageError;

pub struct GCSStorage(GoogleCloudStorage);

#[async_trait]
impl Storage for GCSStorage {
    async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        self.storage()
            .get(&Self::try_path_from(key)?)
            .await?
            .bytes()
            .await
            .map_err(StorageError::from)
    }

    async fn get_with_meta(&self, key: &str) -> Result<ObjectWithMeta, StorageError> {
        let result = self.storage().get(&Self::try_path_from(key)?).await?;
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
            .put_opts(
                &Self::try_path_from(key)?,
                object.into(),
                PutMode::Create.into(),
            )
            .await?;
        Ok(())
    }

    async fn put_overwrite(&self, key: &str, object: Bytes) -> Result<(), StorageError> {
        self.storage()
            .put_opts(
                &Self::try_path_from(key)?,
                object.into(),
                PutMode::Overwrite.into(),
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

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        let prefix = if prefix.is_empty() {
            None
        } else {
            Some(Self::try_path_from(prefix)?)
        };
        let mut stream = self.storage().list(prefix.as_ref());
        let mut keys = Vec::new();
        while let Some(meta) = stream.next().await {
            keys.push(meta?.location.to_string());
        }
        Ok(keys)
    }
}

impl GCSStorage {
    fn try_path_from(key: &str) -> Result<Path, object_store::path::Error> {
        Path::from_url_path(key)
    }

    fn storage(&self) -> &GoogleCloudStorage {
        &self.0
    }
}

impl TryFrom<(&str, &Settings)> for GCSStorage {
    type Error = StorageError;

    fn try_from((bucket, settings): (&str, &Settings)) -> Result<Self, Self::Error> {
        // NOTE: `with_client_options` replaces the builder's internal ClientOptions entirely
        // (see the same note in `s3_storage.rs`). `GoogleCloudStorageBuilder::default()`
        // sets `allow_http(true)`, so overriding it here without carrying the setting over
        // would make any `http://` endpoint override (e.g. fake-gcs-server in tests) fail
        // instantly with a client-side "builder error" (reqwest's scheme check), before any
        // request is even sent.
        let client_options = ClientOptions::new()
            .with_connect_timeout(Duration::from_secs(settings.gcs.connect_timeout_seconds))
            .with_timeout(Duration::from_secs(settings.gcs.request_timeout_seconds))
            .with_allow_http(settings.gcs.allow_http);

        let mut gcs = if settings.gcs.skip_signature {
            // Unauthenticated target (e.g. fake-gcs-server in tests). `from_env()` would
            // still perform real Application Default Credentials discovery (gcloud ADC file
            // / metadata server) inside `build()`, and `with_skip_signature` alone doesn't
            // skip the OAuth2 token fetch on the PUT path (object_store 0.14's PUT request
            // path doesn't consult `skip_signature`, only GET/DELETE/HEAD do). A
            // `disable_oauth` service account key makes every credential provider return a
            // static empty bearer token with no network calls at all, which is what
            // object_store's own test suite uses against fake-gcs-server.
            const FAKE_SERVICE_ACCOUNT_KEY: &str = r#"{"private_key":"unused","private_key_id":"unused","client_email":"unused@example.com","disable_oauth":true}"#;
            GoogleCloudStorageBuilder::new()
                .with_service_account_key(FAKE_SERVICE_ACCOUNT_KEY)
                .with_skip_signature(true)
        } else {
            GoogleCloudStorageBuilder::from_env()
        };
        if let Some(endpoint) = &settings.gcs.endpoint {
            gcs = gcs.with_base_url(endpoint);
        }
        let gcs = gcs
            .with_bucket_name(bucket)
            .with_client_options(client_options);
        Ok(Self(gcs.build()?))
    }
}
