use crate::cached_crate_storage::CachedCrateStorage;
use rocket::tokio;
use settings::Settings;
use std::ops::{Deref, DerefMut};

pub struct KellnrCrateStorage(CachedCrateStorage);

impl KellnrCrateStorage {
    pub async fn new(settings: &Settings) -> Result<Self, anyhow::Error> {
        Ok(Self(CachedCrateStorage::new(settings).await?))
    }

    pub async fn delete(&self, crate_name: &str, crate_version: &str) -> Result<(), anyhow::Error> {
        let path = self.0.crate_path(crate_name, crate_version);
        tokio::fs::remove_file(path).await?;
        Ok(())
    }
}

impl Deref for KellnrCrateStorage {
    type Target = CachedCrateStorage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KellnrCrateStorage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod bin_tests {
    use super::*;
    use crate::pub_data::PubData;
    use common::original_name::OriginalName;
    use common::publish_metadata::PublishMetadata;
    use common::version::Version;
    use rocket::{
        async_test,
        tokio::{fs::File, io::AsyncReadExt},
    };
    use settings::Settings;
    use std::{convert::TryFrom, path::Path};

    struct TestBin {
        settings: Settings,
        crate_storage: KellnrCrateStorage,
    }

    impl TestBin {
        async fn from(data_dir: &str) -> TestBin {
            let settings = Settings {
                data_dir: data_dir.to_string(),
                admin_pwd: String::new(),
                session_age_seconds: 60,
                ..Settings::new().unwrap()
            };
            //fs::create_dir_all(&settings.bin_path()).expect("Cannot create test bin directory.");
            let crate_storage = KellnrCrateStorage::new(&settings).await.unwrap();
            TestBin {
                settings,
                crate_storage,
            }
        }
    }

    impl Drop for TestBin {
        fn drop(&mut self) {
            rm_rf::remove(&self.settings.data_dir).expect("Cannot remove test bin directory.");
        }
    }

    #[async_test]
    async fn add_crate_binary() {
        let pub_data = PubData {
            crate_length: 5,
            cratedata: vec![0x00, 0x11, 0x22, 0x33, 0x44],
            metadata_length: 0,
            metadata: PublishMetadata::minimal("test", "0.1.0"),
        };

        let test_storage = TestBin::from("test_add_crate_binary").await;
        let name = OriginalName::try_from("test").unwrap();
        let version = Version::try_from("0.1.0").unwrap();
        let result = test_storage
            .crate_storage
            .add_bin_package(&name, &version, &pub_data.cratedata)
            .await;
        let result_crate = Path::new(&test_storage.settings.bin_path()).join("test-0.1.0.crate");

        let mut file = File::open(&result_crate)
            .await
            .expect("Cannot open written test crate");
        let mut data: Vec<u8> = Vec::new();
        file.read_to_end(&mut data)
            .await
            .expect("Cannot read written test crate.");

        assert!(result.is_ok());
        assert!(result_crate.exists());
        assert_eq!(vec![0x00, 0x11, 0x22, 0x33, 0x44], data);
    }

    #[async_test]
    async fn add_crate_binary_with_upper_case_name() {
        let pub_data = PubData {
            crate_length: 5,
            cratedata: vec![0x00, 0x11, 0x22, 0x33, 0x44],
            metadata_length: 0,
            metadata: PublishMetadata::minimal("Test_Add_crate_binary_Upper-Case", "0.1.0"),
        };

        let test_storage = TestBin::from("Test_Add_crate_binary_Upper-Case").await;
        let name = OriginalName::try_from(pub_data.metadata.name).unwrap();
        let version = Version::try_from("0.1.0").unwrap();
        let result = test_storage
            .crate_storage
            .add_bin_package(&name, &version, &pub_data.cratedata)
            .await;
        let result_crate = Path::new(&test_storage.settings.bin_path())
            .join("Test_Add_crate_binary_Upper-Case-0.1.0.crate");

        let mut file = File::open(&result_crate)
            .await
            .expect("Cannot open written test crate");
        let mut data: Vec<u8> = Vec::new();
        file.read_to_end(&mut data)
            .await
            .expect("Cannot read written test crate.");

        assert!(result.is_ok());
        assert!(result_crate.exists());
        assert_eq!(vec![0x00, 0x11, 0x22, 0x33, 0x44], data);
    }

    #[async_test]
    async fn add_duplicate_crate_binary() {
        let pub_data = PubData {
            crate_length: 5,
            cratedata: vec![0x00, 0x11, 0x22, 0x33, 0x44],
            metadata_length: 0,
            metadata: PublishMetadata::minimal("test", "0.1.0"),
        };

        let test_bin = TestBin::from("test_add_duplicate_crate_binary").await;
        let name = OriginalName::try_from("test").unwrap();
        let version = Version::try_from("0.1.0").unwrap();

        let _ = test_bin
            .crate_storage
            .add_bin_package(&name, &version, &pub_data.cratedata)
            .await;
        let result = test_bin
            .crate_storage
            .add_bin_package(&name, &version, &pub_data.cratedata)
            .await;

        assert!(result.is_err());
        assert_eq!(
            "Crate with version already exists: test-0.1.0",
            result.unwrap_err().to_string()
        );
    }

    #[async_test]
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
    }

    #[async_test]
    async fn delete_crate() {
        let pub_data = PubData {
            crate_length: 5,
            cratedata: vec![0x00, 0x11, 0x22, 0x33, 0x44],
            metadata_length: 0,
            metadata: PublishMetadata::minimal("test", "0.1.0"),
        };
        let test_storage = TestBin::from("test_delete").await;
        let name = OriginalName::try_from("test").unwrap();
        let version = Version::try_from("0.1.0").unwrap();
        test_storage
            .crate_storage
            .add_bin_package(&name, &version, &pub_data.cratedata)
            .await
            .unwrap();
        let crate_path = Path::new(&test_storage.settings.bin_path()).join("test-0.1.0.crate");

        test_storage
            .crate_storage
            .delete(&name, &version)
            .await
            .unwrap();

        assert!(!crate_path.exists());
    }
}
