use common::original_name::OriginalName;
use common::publish_metadata::PublishMetadata;
use common::version::Version;
use minio_testcontainer::*;
use settings::Settings;
use settings::s3::S3;
use std::sync::Arc;
use std::{convert::TryFrom, path::Path};
use storage::cached_crate_storage::DynStorage;
use storage::kellnr_crate_storage::KellnrCrateStorage;
use storage::s3_storage::S3Storage;
mod image;

struct TestData {
    settings: Settings,
    crate_storage: KellnrCrateStorage,
}

impl TestData {
    async fn from(data_dir: &str, url: &str) -> TestData {
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
        let storage = Box::new(S3Storage::try_from((settings.crates_path(), &settings)).unwrap())
            as DynStorage;
        let crate_storage = KellnrCrateStorage::new(&settings, storage).await.unwrap();
        TestData {
            settings,
            crate_storage,
        }
    }
}

// async fn connect_s3() -> ContainerAsync<impl Image> {
//     let container = GenericImage::new("minio/minio", "latest")
//         .with_entrypoint("sh")
//         .with_wait_for(WaitFor::message_on_stderr("API:"))
//         .with_exposed_port(9000.tcp())
//         .with_exposed_port(9001.tcp())
//         .with_mapped_port(9000, 9000.tcp())
//         .with_mapped_port(9001, 9001.tcp())
//         .with_env_var("MINIO_ROOT_USER", "minioadmin")
//         .with_env_var("MINIO_ROOT_PASSWORD", "minioadmin")
//         .with_env_var("MINIO_CONSOLE_ADDRESS", ":9001")
//         .with_log_consumer(LoggingConsumer::default().with_prefix("s3-container"))
//         .with_cmd(["-c", "mkdir -p /data/crates && /usr/bin/minio server /data"])
//         .pull_image()
//         .await
//         .expect("Failed to start s3");
//
//     container.start().await.expect("Failed to start s3")
// }

#[minio_testcontainer]
#[tokio::test]
async fn add_and_get_crate() {
    let host = container.get_host().await.unwrap().to_string();
    let url = format!("http://{}:{}", host, port);
    let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);
    let metadata = PublishMetadata::minimal("Test_Add_crate_binary_Upper-Case", "0.1.0");
    let test_storage = TestData::from("Test_Add_crate_binary_Upper-Case", &url).await;
    let name = OriginalName::try_from(metadata.name).unwrap();
    let version = Version::try_from("0.1.0").unwrap();
    let path = test_storage
        .crate_storage
        .crate_path(&name.to_string(), &version.to_string());

    // Put the crate into the S3 storage
    let put_result = test_storage
        .crate_storage
        .put(&name, &version, cratedata)
        .await;

    // Get the crate from the S3 storage
    let result_crate = test_storage.crate_storage.get(path.as_str()).await;

    assert!(put_result.is_ok());
    assert!(result_crate.is_some());
    assert_eq!(Some(vec![0x00, 0x11, 0x22, 0x33, 0x44]), result_crate);
}

#[minio_testcontainer]
#[tokio::test]
async fn remove_crate() {
    let host = container.get_host().await.unwrap().to_string();
    let url = format!("http://{}:{}", host, port);
    let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);
    let test_storage = TestData::from("test_delete", &url).await;
    let name = OriginalName::try_from("test").unwrap();
    let version = Version::try_from("0.1.0").unwrap();
    test_storage
        .crate_storage
        .put(&name, &version, cratedata)
        .await
        .unwrap();

    let res = test_storage.crate_storage.delete(&name, &version).await;

    assert!(res.is_ok());
}
