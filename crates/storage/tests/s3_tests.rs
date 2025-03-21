use testcontainers::core::logs::consumer::logging_consumer::LoggingConsumer;
use testcontainers::{GenericImage, Image, ImageExt};

use common::original_name::OriginalName;
use common::publish_metadata::PublishMetadata;
use common::version::Version;
use settings::Settings;
use settings::s3::S3;
use std::sync::Arc;
use std::{convert::TryFrom, path::Path};
use storage::kellnr_crate_storage::KellnrCrateStorage;
use testcontainers::ContainerAsync;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;

struct TestBin {
    settings: Settings,
    crate_storage: KellnrCrateStorage,
}

impl TestBin {
    async fn from(data_dir: &str, url: &str) -> TestBin {
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
                access_key: "minioadmin".into(),
                secret_key: "minioadmin".into(),
                endpoint: url.to_string(),
                ..S3::default()
            },
            ..Settings::default()
        };
        let crate_storage = KellnrCrateStorage::new(&settings).await.unwrap();
        TestBin {
            settings,
            crate_storage,
        }
    }
}

async fn connect_s3() -> ContainerAsync<impl Image> {
    let container = GenericImage::new("minio/minio", "latest")
        .with_entrypoint("sh")
        .with_wait_for(WaitFor::message_on_stderr("API:"))
        .with_exposed_port(9000.tcp())
        .with_exposed_port(9001.tcp())
        .with_mapped_port(9000, 9000.tcp())
        .with_mapped_port(9001, 9001.tcp())
        .with_env_var("MINIO_ROOT_USER", "minioadmin")
        .with_env_var("MINIO_ROOT_PASSWORD", "minioadmin")
        .with_env_var("MINIO_CONSOLE_ADDRESS", ":9001")
        .with_log_consumer(LoggingConsumer::default().with_prefix("s3-container"))
        .with_cmd(["-c", "mkdir -p /data/crates && /usr/bin/minio server /data"])
        .pull_image()
        .await
        .expect("Failed to start s3");

    container.start().await.expect("Failed to start s3")
}

#[tokio::test]
async fn add_remove_crate() {
    let container = connect_s3().await;
    let host = container.get_host().await.unwrap().to_string();
    let port = container
        .get_host_port_ipv4(9000)
        .await
        .unwrap()
        .to_string();
    let host = format!("http://{}:{}", host, port);

    let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

    let test_storage = TestBin::from("test_add_crate_binary", &host).await;
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

    let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

    let metadata = PublishMetadata::minimal("Test_Add_crate_binary_Upper-Case", "0.1.0");

    let test_storage = TestBin::from("Test_Add_crate_binary_Upper-Case", &host).await;
    let name = OriginalName::try_from(metadata.name).unwrap();
    let version = Version::try_from("0.1.0").unwrap();
    let put_result = test_storage
        .crate_storage
        .add_bin_package(&name, &version, cratedata)
        .await;
    let path = test_storage
        .crate_storage
        .crate_path(&name.to_string(), &version.to_string());

    let result_crate = test_storage.crate_storage.get_file(path.as_str()).await;

    assert!(put_result.is_ok());
    assert!(result_crate.is_some());
    assert_eq!(Some(vec![0x00, 0x11, 0x22, 0x33, 0x44]), result_crate);

    let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);

    let test_storage = TestBin::from("test_delete", &host).await;
    let name = OriginalName::try_from("test").unwrap();
    let version = Version::try_from("0.1.0").unwrap();
    test_storage
        .crate_storage
        .add_bin_package(&name, &version, cratedata)
        .await
        .unwrap();

    let res = test_storage.crate_storage.delete(&name, &version).await;

    assert!(res.is_ok());

    container.stop().await.expect("Failed to stop");
    container.rm().await.expect("Failed to remove container");
}
