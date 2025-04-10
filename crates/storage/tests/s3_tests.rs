use common::original_name::OriginalName;
use common::publish_metadata::PublishMetadata;
use common::version::Version;
use minio_testcontainer::*;
use settings::Settings;
use settings::s3::S3;
use std::convert::TryFrom;
use std::sync::Arc;
use storage::cached_crate_storage::DynStorage;
use storage::kellnr_crate_storage::KellnrCrateStorage;
use storage::s3_storage::S3Storage;
mod image;

struct TestS3Storage {
    crate_storage: KellnrCrateStorage,
}

impl TestS3Storage {
    async fn from(data_dir: &str, url: &str) -> TestS3Storage {
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
        let storage =
            Box::new(S3Storage::try_from((settings.s3.crates_bucket.as_str(), &settings)).unwrap())
                as DynStorage;
        let crate_storage = KellnrCrateStorage::new(&settings, storage).await.unwrap();
        TestS3Storage { crate_storage }
    }
}

#[minio_testcontainer]
#[tokio::test]
async fn add_and_get_crate() {
    let host = container.get_host().await.unwrap().to_string();
    let url = format!("http://{}:{}", host, port);
    let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);
    let metadata = PublishMetadata::minimal("Test_Add_crate_binary_Upper-Case", "0.1.0");
    let test_storage = TestS3Storage::from("Test_Add_crate_binary_Upper-Case", &url).await;
    let name = OriginalName::try_from(metadata.name).unwrap();
    let version = Version::try_from("0.1.0").unwrap();

    // Put the crate into the S3 storage
    let put_result = test_storage
        .crate_storage
        .put(&name, &version, cratedata)
        .await;

    // Get the crate from the S3 storage
    let result_crate = test_storage.crate_storage.get(&name, &version).await;

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
    let test_storage = TestS3Storage::from("test_delete", &url).await;
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
