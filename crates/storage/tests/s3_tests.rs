use std::borrow::Cow;
use std::collections::HashMap;
use testcontainers::core::wait::HttpWaitStrategy;
use testcontainers::core::{ContainerPort, IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::Image;

const NAME: &str = "minio/minio";
const TAG: &str = "latest";
const MINIO_ROOT_USER: &str = "minioadmin";
const MINIO_ROOT_PASSWORD: &str = "minioadmin";

#[derive(Debug, Clone)]
pub struct S3Image {
    env_vars: HashMap<String, String>,
}

impl S3Image {
    pub const PORT: ContainerPort = ContainerPort::Tcp(9000);
}

impl Default for S3Image {
    fn default() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("MINIO_ROOT_USER".to_owned(), MINIO_ROOT_USER.to_owned());
        env_vars.insert(
            "MINIO_ROOT_PASSWORD".to_owned(),
            MINIO_ROOT_PASSWORD.to_owned(),
        );
        Self { env_vars }
    }
}

impl Image for S3Image {
    fn expose_ports(&self) -> &[testcontainers::core::ContainerPort] {
        &[S3Image::PORT]
    }
    fn name(&self) -> &str {
        NAME
    }

    fn tag(&self) -> &str {
        TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::Http(
            HttpWaitStrategy::new("http://localhost:9000/minio/health/live")
                .with_response_matcher(|response| response.status().is_success()),
        )]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        self.env_vars.clone()
    }
}

#[cfg(test)]
mod bin_tests {

    use common::original_name::OriginalName;
    use common::publish_metadata::PublishMetadata;
    use common::version::Version;
    use settings::s3::S3;
    use settings::Settings;
    use testcontainers::runners::AsyncRunner;
    use std::sync::Arc;
    use std::{convert::TryFrom, path::Path};
    use storage::kellnr_crate_storage::KellnrCrateStorage;

    use crate::S3Image;

    struct TestBin {
        settings: Settings,
        crate_storage: KellnrCrateStorage,
    }

    static CONTAINER: S3Image = S3Image::default();

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
                s3: S3 {
                    enabled: true,
                    access_key: "kekek".into(),
                    secret_key: "kffwqO0RBK6jL33KMJ1Fgdew70ddxRbDRt7ExFBO".into(),
                    ..S3::default()
                },
                ..Settings::default()
            };
            //fs::create_dir_all(&settings.bin_path()).expect("Cannot create test bin directory.");
            let crate_storage = KellnrCrateStorage::new(&settings).await.unwrap();
            TestBin {
                settings,
                crate_storage,
            }
        }
    }

    async fn connect_s3() {
        let _ = 
            .start()
            .await
            .expect("Failed to start s3");
    }

    #[tokio::test]
    async fn add_crate_binary() {
        connect_s3().await;
        let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

        let test_storage = TestBin::from("test_add_crate_binary").await;
        let name = OriginalName::try_from("test").unwrap();
        let version = Version::try_from("0.1.0").unwrap();
        let result = test_storage
            .crate_storage
            .add_bin_package(&name, &version, cratedata)
            .await;

        let path = Path::new(&test_storage.settings.bin_path()).join("test-0.1.0.crate");
        let path = path.as_os_str().to_str().unwrap();

        let result_crate = test_storage.crate_storage.get_file(path).await;

        assert!(result.is_ok());
        assert!(result_crate.is_some());
        assert_eq!(Some(vec![0x00, 0x11, 0x22, 0x33, 0x44]), result_crate);
    }

    #[tokio::test]
    async fn add_crate_binary_with_upper_case_name() {
        connect_s3().await;
        let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

        let metadata = PublishMetadata::minimal("Test_Add_crate_binary_Upper-Case", "0.1.0");

        let test_storage = TestBin::from("Test_Add_crate_binary_Upper-Case").await;
        let name = OriginalName::try_from(metadata.name).unwrap();
        let version = Version::try_from("0.1.0").unwrap();
        let result = test_storage
            .crate_storage
            .add_bin_package(&name, &version, cratedata)
            .await;
        let path = test_storage
            .crate_storage
            .crate_path(&name.to_string(), &version.to_string());

        let result_crate = test_storage.crate_storage.get_file(path.as_str()).await;

        assert!(result.is_ok());
        assert!(result_crate.is_some());
        assert_eq!(Some(vec![0x00, 0x11, 0x22, 0x33, 0x44]), result_crate);
    }

    #[tokio::test]
    async fn deleting_crate() {
        connect_s3().await;
        let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

        let test_storage = TestBin::from("test_delete").await;
        let name = OriginalName::try_from("test").unwrap();
        let version = Version::try_from("0.1.0").unwrap();
        test_storage
            .crate_storage
            .add_bin_package(&name, &version, cratedata)
            .await
            .unwrap();

        let res = test_storage.crate_storage.delete(&name, &version).await;

        assert!(res.is_ok());
    }
}
