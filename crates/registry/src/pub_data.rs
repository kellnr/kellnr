use std::sync::Arc;

use crate::registry_error::RegistryError;
use appstate::AppStateData;
use axum::body::{Body, Bytes};
use axum::extract::FromRequest;
use axum::http::Request;
use common::publish_metadata::PublishMetadata;
use error::api_error::ApiError;
use settings::constants::MIN_BODY_CRATE_AND_DOC_BYTES;

#[derive(Debug, PartialEq, Eq)]
pub struct PubData {
    pub metadata_length: u32,
    pub metadata: PublishMetadata,
    pub crate_length: u32,
    pub cratedata: Arc<[u8]>,
}

fn convert_raw_metadata_to_string(raw_data: &[u8]) -> Result<String, RegistryError> {
    Ok(String::from_utf8((raw_data).to_vec())?)
}

fn deserialize_metadata(raw_data: &[u8]) -> Result<PublishMetadata, RegistryError> {
    let metadata_string = convert_raw_metadata_to_string(raw_data)?;
    Ok(serde_json::from_str(&metadata_string)?)
}

fn convert_length(raw_data: &[u8]) -> Result<u32, RegistryError> {
    match std::convert::TryInto::try_into(raw_data) {
        Ok(i) => Ok(u32::from_le_bytes(i)),
        Err(e) => Err(RegistryError::InvalidMetadataLength(e)),
    }
}

#[axum::async_trait]
impl FromRequest<AppStateData, Body> for PubData {
    type Rejection = ApiError;

    async fn from_request(
        req: Request<Body>,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let data_bytes: Vec<u8> = Bytes::from_request(req, state)
            .await
            .map_err(RegistryError::ExtractBytesFailed)?
            .to_vec();

        if data_bytes.len() < MIN_BODY_CRATE_AND_DOC_BYTES {
            return Err(RegistryError::InvalidMinLength(
                data_bytes.len(),
                MIN_BODY_CRATE_AND_DOC_BYTES,
            )
            .into());
        }

        let metadata_length = convert_length(&data_bytes[0..4])?;
        let metadata_end = 4 + (metadata_length as usize);

        if metadata_end >= data_bytes.len() {
            return Err(RegistryError::InvalidMetadataSize.into());
        }

        let metadata: PublishMetadata = deserialize_metadata(&data_bytes[4..metadata_end])?;
        let crate_length = convert_length(&data_bytes[metadata_end..(metadata_end + 4)])?;
        let crate_end = metadata_end + 4 + (crate_length as usize);
        let cratedata = Arc::from(data_bytes[metadata_end + 4..crate_end].to_vec());

        let pub_data = PubData {
            metadata_length,
            metadata,
            crate_length,
            cratedata,
        };

        Ok(pub_data)
    }
}

#[cfg(test)]
mod bin_tests {
    use crate::pub_data::PubData;
    use common::original_name::OriginalName;
    use common::publish_metadata::PublishMetadata;
    use common::version::Version;
    use settings::Settings;

    use std::{convert::TryFrom, path::Path};
    use storage::kellnr_crate_storage::KellnrCrateStorage;
    use tokio::fs::File;
    use tokio::io::AsyncReadExt;

    struct TestBin {
        settings: Settings,
        crate_storage: KellnrCrateStorage,
    }

    impl TestBin {
        async fn from(data_dir: &str) -> TestBin {
            let settings = Settings {
                registry: settings::Registry {
                    data_dir: data_dir.to_owned(),
                    session_age_seconds: 60,
                    ..settings::Registry::default()
                },
                setup: settings::Setup {
                    admin_pwd: String::new(),
                    ..settings::Setup::default()
                },
                ..Settings::default()
            };
            let crate_storage = KellnrCrateStorage::new(&settings).await.unwrap();
            TestBin {
                settings,
                crate_storage,
            }
        }
        fn clean(&self) {
            rm_rf::ensure_removed(&self.settings.registry.data_dir)
                .expect("Cannot remove test bin directory.");
        }
    }

    #[tokio::test]
    async fn add_crate_binary() {
        let pub_data = PubData {
            crate_length: 5,
            cratedata: vec![0x00, 0x11, 0x22, 0x33, 0x44].into(),
            metadata_length: 0,
            metadata: PublishMetadata::minimal("test", "0.1.0"),
        };

        let test_storage = TestBin::from("test_add_crate_binary").await;
        let name = OriginalName::try_from("test").unwrap();
        let version = Version::try_from("0.1.0").unwrap();
        let result = test_storage
            .crate_storage
            .add_bin_package(&name, &version, pub_data.cratedata.clone())
            .await;
        let result_crate = Path::new(&test_storage.settings.bin_path()).join("test-0.1.0.crate");

        let get_res = test_storage
            .crate_storage
            .get_file(result_crate.to_str().unwrap())
            .await
            .expect("Couldn't find file...");

        assert!(result.is_ok());
        assert_eq!(vec![0x00, 0x11, 0x22, 0x33, 0x44], get_res);

        test_storage.clean();
    }

    #[tokio::test]
    async fn add_crate_binary_with_upper_case_name() {
        let pub_data = PubData {
            crate_length: 5,
            cratedata: vec![0x00, 0x11, 0x22, 0x33, 0x44].into(),
            metadata_length: 0,
            metadata: PublishMetadata::minimal("Test_Add_crate_binary_Upper-Case", "0.1.0"),
        };

        let test_storage = TestBin::from("Test_Add_crate_binary_Upper-Case").await;
        let name = OriginalName::try_from(pub_data.metadata.name).unwrap();
        let version = Version::try_from("0.1.0").unwrap();
        let result = test_storage
            .crate_storage
            .add_bin_package(&name, &version, pub_data.cratedata.clone())
            .await;
        let result_crate = Path::new(&test_storage.settings.bin_path())
            .join("Test_Add_crate_binary_Upper-Case-0.1.0.crate");

        let get_res = test_storage
            .crate_storage
            .get_file(result_crate.to_str().unwrap())
            .await
            .expect("Couldn't find file...");

        assert!(result.is_ok());

        assert_eq!(vec![0x00, 0x11, 0x22, 0x33, 0x44], get_res);
        test_storage.clean();
    }

    #[tokio::test]
    async fn add_duplicate_crate_binary() {
        let pub_data = PubData {
            crate_length: 5,
            cratedata: vec![0x00, 0x11, 0x22, 0x33, 0x44].into(),
            metadata_length: 0,
            metadata: PublishMetadata::minimal("test", "0.1.0"),
        };

        let test_bin = TestBin::from("test_add_duplicate_crate_binary").await;
        let name = OriginalName::try_from("test").unwrap();
        let version = Version::try_from("0.1.0").unwrap();

        let _ = test_bin
            .crate_storage
            .add_bin_package(&name, &version, pub_data.cratedata.clone())
            .await;
        let result = test_bin
            .crate_storage
            .add_bin_package(&name, &version, pub_data.cratedata.clone())
            .await;

        assert!(result.is_err());
        assert_eq!(
            "Crate with version already exists: test-0.1.0",
            result.unwrap_err().to_string()
        );
        test_bin.clean();
    }

    #[tokio::test]
    async fn create_rand_doc_queue_path() {
        let test_bin = TestBin::from("test_doc_queue").await;

        let rand_path = test_bin
            .crate_storage
            .create_rand_doc_queue_path()
            .await
            .unwrap();

        assert!(rand_path.exists());
        assert!(rand_path.starts_with(
            test_bin
                .crate_storage
                .doc_queue_path
                .to_string_lossy()
                .to_string()
        ));
        test_bin.clean();
    }

    #[tokio::test]
    async fn deleting_crate() {
        let pub_data = PubData {
            crate_length: 5,
            cratedata: vec![0x00, 0x11, 0x22, 0x33, 0x44].into(),
            metadata_length: 0,
            metadata: PublishMetadata::minimal("test", "0.1.0"),
        };
        let test_storage = TestBin::from("test_delete").await;
        let name = OriginalName::try_from("test").unwrap();
        let version = Version::try_from("0.1.0").unwrap();
        test_storage
            .crate_storage
            .add_bin_package(&name, &version, pub_data.cratedata.clone())
            .await
            .unwrap();
        let crate_path = Path::new(&test_storage.settings.bin_path()).join("test-0.1.0.crate");

        test_storage
            .crate_storage
            .delete(&name, &version)
            .await
            .unwrap();

        assert!(!crate_path.exists());
        test_storage.clean();
    }

    #[tokio::test]
    async fn delete_crate_invalidates_cache() {
        let pub_data = PubData {
            crate_length: 5,
            cratedata: vec![0x00, 0x11, 0x22, 0x33, 0x44].into(),
            metadata_length: 0,
            metadata: PublishMetadata::minimal("test", "0.2.0"),
        };

        let test_storage = TestBin::from("test_delete").await;
        let name = OriginalName::try_from("test").unwrap();
        let version = Version::try_from("0.2.0").unwrap();

        test_storage
            .crate_storage
            .add_bin_package(&name, &version, pub_data.cratedata.clone())
            .await
            .unwrap();

        let crate_path = Path::new(&test_storage.settings.bin_path()).join("test-0.2.0.crate");

        let crate_path_str = crate_path.to_str().unwrap();
        assert!(test_storage
            .crate_storage
            .get_file(crate_path_str)
            .await
            .is_some());

        assert!(test_storage.crate_storage.cache_has_path(&crate_path));

        test_storage
            .crate_storage
            .delete(&name, &version)
            .await
            .unwrap();

        assert!(!test_storage.crate_storage.cache_has_path(&crate_path));
        assert!(test_storage
            .crate_storage
            .get_file(crate_path_str)
            .await
            .is_none());
        assert!(!crate_path.exists());
        test_storage.clean();
    }
}
