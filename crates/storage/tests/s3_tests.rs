// use std::borrow::Cow;
// use std::collections::HashMap;
// use testcontainers::core::wait::HttpWaitStrategy;
// use testcontainers::core::WaitFor;
// use testcontainers::Image;

// const NAME: &str = " minio/minio";
// const TAG: &str = "latest";
// const MINIO_ROOT_USER: &str = "minioadmin";
// const MINIO_ROOT_PASSWORD: &str = "minioadmin";

// #[derive(Debug, Clone)]
// pub struct S3Image {
//     env_vars: HashMap<String, String>,
// }

// impl S3Image {
//     pub const PG_PORT: u16 = 5432;
// }

// impl Default for S3Image {
//     fn default() -> Self {
//         let mut env_vars = HashMap::new();
//         env_vars.insert("MINIO_ROOT_USER".to_owned(), MINIO_ROOT_USER.to_owned());
//         env_vars.insert(
//             "MINIO_ROOT_PASSWORD".to_owned(),
//             MINIO_ROOT_PASSWORD.to_owned(),
//         );
//         Self { env_vars }
//     }
// }

// impl Image for S3Image {
//     fn name(&self) -> &str {
//         NAME
//     }

//     fn tag(&self) -> &str {
//         TAG
//     }

//     fn ready_conditions(&self) -> Vec<WaitFor> {
//         vec![WaitFor::Http(HttpWaitStrategy::new(
//             "http://localhost:9000/minio/health/live",
//         ))]
//     }

//     fn env_vars(
//         &self,
//     ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
//         self.env_vars.clone()
//     }
// }

// #[cfg(test)]
// mod bin_tests {

//     use common::original_name::OriginalName;
//     use common::publish_metadata::PublishMetadata;
//     use common::version::Version;
//     use settings::s3::S3;
//     use settings::Settings;
//     use std::sync::Arc;
//     use std::{convert::TryFrom, path::Path};
//     use storage::kellnr_crate_storage::KellnrCrateStorage;

//     struct TestBin {
//         settings: Settings,
//         crate_storage: KellnrCrateStorage,
//     }

//     impl TestBin {
//         async fn from(data_dir: &str) -> TestBin {
//             let settings = Settings {
//                 registry: settings::Registry {
//                     data_dir: data_dir.to_owned(),
//                     session_age_seconds: 60,
//                     ..settings::Registry::default()
//                 },
//                 setup: settings::Setup {
//                     admin_pwd: String::new(),
//                     ..settings::Setup::default()
//                 },
//                 s3: S3 {
//                     enabled: true,
//                     ..S3::default()
//                 },
//                 ..Settings::default()
//             };
//             //fs::create_dir_all(&settings.bin_path()).expect("Cannot create test bin directory.");
//             let crate_storage = KellnrCrateStorage::new(&settings).await.unwrap();
//             TestBin {
//                 settings,
//                 crate_storage,
//             }
//         }
//         fn clean(&self) {
//             // rm_rf::ensure_removed(&self.settings.registry.data_dir)
//             // .expect("Cannot remove test bin directory.");
//         }
//     }

//     async fn connect_s3() {
//         // use testcontainers::runners::AsyncRunner;
//         // let _ = S3Image::default().start().await.expect("Failed to start s3");
//     }

//     #[tokio::test]
//     async fn add_crate_binary() {
//         connect_s3().await;
//         let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

//         let test_storage = TestBin::from("test_add_crate_binary").await;
//         let name = OriginalName::try_from("test").unwrap();
//         let version = Version::try_from("0.1.0").unwrap();
//         let result = test_storage
//             .crate_storage
//             .add_bin_package(&name, &version, cratedata)
//             .await;
//         let result_crate = Path::new(&test_storage.settings.bin_path()).join("test-0.1.0.crate");

//         let path = Path::new(&test_storage.settings.bin_path()).join("test-0.1.0.crate");
//         let path = path.to_str().unwrap();

//         let result_crate = test_storage.crate_storage.get_file(path).await;

//         assert!(result.is_ok());
//         assert!(result_crate.is_some());
//         assert_eq!(Some(vec![0x00, 0x11, 0x22, 0x33, 0x44]), result_crate);

//         test_storage.clean();
//     }
//     //TODO :FINISH
//     #[tokio::test]
//     async fn add_crate_binary_with_upper_case_name() {
//         connect_s3().await;
//         let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

//         let metadata = PublishMetadata::minimal("Test_Add_crate_binary_Upper-Case", "0.1.0");

//         let test_storage = TestBin::from("Test_Add_crate_binary_Upper-Case").await;
//         let name = OriginalName::try_from(metadata.name).unwrap();
//         let version = Version::try_from("0.1.0").unwrap();
//         let result = test_storage
//             .crate_storage
//             .add_bin_package(&name, &version, cratedata)
//             .await;
//         let path = test_storage
//             .crate_storage
//             .crate_path(&name.to_string(), &version.to_string());

//         let result_crate = test_storage.crate_storage.get_file(path.as_str()).await;

//         assert!(result.is_ok());
//         assert!(result_crate.is_some());
//         assert_eq!(Some(vec![0x00, 0x11, 0x22, 0x33, 0x44]), result_crate);
//         test_storage.clean();
//     }

//     #[tokio::test]
//     async fn add_duplicate_crate_binary() {
//         connect_s3().await;
//         let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

//         let test_bin = TestBin::from("test_add_duplicate_crate_binary").await;
//         let name = OriginalName::try_from("test").unwrap();
//         let version = Version::try_from("0.1.0").unwrap();

//         let _ = test_bin
//             .crate_storage
//             .add_bin_package(&name, &version, cratedata.clone())
//             .await;
//         let result = test_bin
//             .crate_storage
//             .add_bin_package(&name, &version, cratedata)
//             .await;

//         assert!(result.is_err());
//         assert_eq!(
//             "Crate with version already exists: test-0.1.0",
//             result.unwrap_err().to_string()
//         );
//         test_bin.clean();
//     }

//     #[tokio::test]
//     async fn create_rand_doc_queue_path() {
//         connect_s3().await;
//         let test_bin = TestBin::from("test_doc_queue").await;

//         let rand_path = test_bin
//             .crate_storage
//             .create_rand_doc_queue_path()
//             .await
//             .unwrap();

//         assert!(rand_path.exists());
//         assert!(rand_path.starts_with(
//             test_bin
//                 .crate_storage
//                 .doc_queue_path
//                 .to_string_lossy()
//                 .to_string()
//         ));
//         test_bin.clean();
//     }

//     #[tokio::test]
//     async fn deleting_crate() {
//         connect_s3().await;
//         let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

//         let test_storage = TestBin::from("test_delete").await;
//         let name = OriginalName::try_from("test").unwrap();
//         let version = Version::try_from("0.1.0").unwrap();
//         test_storage
//             .crate_storage
//             .add_bin_package(&name, &version, cratedata)
//             .await
//             .unwrap();
//         let crate_path = Path::new(&test_storage.settings.bin_path()).join("test-0.1.0.crate");

//         test_storage
//             .crate_storage
//             .delete(&name, &version)
//             .await
//             .unwrap();

//         assert!(!crate_path.exists());
//         test_storage.clean();
//     }

//     #[tokio::test]
//     async fn delete_crate_invalidates_cache() {
//         connect_s3().await;
//         let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

//         let test_storage = TestBin::from("test_delete").await;
//         let name = OriginalName::try_from("test").unwrap();
//         let version = Version::try_from("0.2.0").unwrap();

//         test_storage
//             .crate_storage
//             .add_bin_package(&name, &version, cratedata)
//             .await
//             .unwrap();

//         let crate_path = Path::new(&test_storage.settings.bin_path()).join("test-0.2.0.crate");

//         assert!(test_storage
//             .crate_storage
//             .get_file(crate_path.clone())
//             .await
//             .is_some());
//         assert!(test_storage.crate_storage.cache_has_path(&crate_path));

//         test_storage
//             .crate_storage
//             .delete(&name, &version)
//             .await
//             .unwrap();

//         assert!(!test_storage.crate_storage.cache_has_path(&crate_path));
//         assert!(test_storage
//             .crate_storage
//             .get_file(crate_path.clone())
//             .await
//             .is_none());
//         assert!(!crate_path.exists());
//         test_storage.clean();
//     }
// }
