use std::time::Duration;

use kellnr_settings::Settings;
use object_store::ClientOptions;
use object_store::gcp::{GoogleCloudStorage, GoogleCloudStorageBuilder};

use crate::object_storage::ObjectStorage;
use crate::storage_error::StorageError;

pub type GCSStorage = ObjectStorage<GoogleCloudStorage>;

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
