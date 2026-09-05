use std::convert::TryFrom;
use std::sync::Arc;

use kellnr_common::original_name::OriginalName;
use kellnr_common::publish_metadata::PublishMetadata;
use kellnr_common::version::Version;
use kellnr_fakegcs_testcontainer::*;
use kellnr_settings::Settings;
use kellnr_settings::gcs::Gcs;
use kellnr_storage::cached_crate_storage::DynStorage;
use kellnr_storage::docs_storage::DocsStorage;
use kellnr_storage::gcs_storage::GCSStorage;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
#[path = "gcs_image.rs"]
mod image;

fn test_settings(data_dir: &str, url: &str) -> Settings {
    Settings {
        registry: kellnr_settings::Registry {
            data_dir: data_dir.to_owned(),
            session_age_seconds: 60,
            ..kellnr_settings::Registry::default()
        },
        setup: kellnr_settings::Setup {
            admin_pwd: String::new(),
            ..kellnr_settings::Setup::default()
        },
        gcs: Gcs {
            enabled: true,
            endpoint: Some(url.to_string()),
            allow_http: true,
            skip_signature: true,
            ..Gcs::default()
        },
        ..Settings::default()
    }
}

struct TestGCSStorage {
    crate_storage: KellnrCrateStorage,
}

impl TestGCSStorage {
    fn from(data_dir: &str, url: &str) -> TestGCSStorage {
        let settings = test_settings(data_dir, url);
        let storage =
            Box::new(GCSStorage::try_from(("kellnr-crates", &settings)).unwrap()) as DynStorage;
        let crate_storage = KellnrCrateStorage::new(&settings, storage);
        TestGCSStorage { crate_storage }
    }
}

fn test_docs_storage(data_dir: &str, url: &str) -> DocsStorage {
    let settings = test_settings(data_dir, url);
    let storage = Box::new(GCSStorage::try_from(("kellnr-docs", &settings)).unwrap()) as DynStorage;
    DocsStorage::new(storage)
}

#[fakegcs_testcontainer]
#[tokio::test]
async fn add_and_get_crate() {
    // `localhost`, not `container.get_host()`: the reserved port is baked into
    // fake-gcs-server's `-public-host` as `localhost:{port}`, and a mismatch makes every
    // object route 404 (see `gcs_image.rs`).
    let url = format!("http://localhost:{port}");
    let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);
    let metadata = PublishMetadata::minimal("Test_Add_crate_binary_Upper-Case", "0.1.0");
    let test_storage = TestGCSStorage::from("Test_Add_crate_binary_Upper-Case_gcs", &url);
    let name = OriginalName::try_from(metadata.name).unwrap();
    let version = Version::try_from("0.1.0").unwrap();

    // Put the crate into the GCS storage
    let put_result = test_storage
        .crate_storage
        .put(&name, &version, cratedata)
        .await;

    // Get the crate from the GCS storage
    let result_crate = test_storage.crate_storage.get(&name, &version).await;

    assert!(put_result.is_ok());
    assert!(result_crate.is_some());
    assert_eq!(
        Some(bytes::Bytes::from_static(&[0x00, 0x11, 0x22, 0x33, 0x44])),
        result_crate
    );
}

#[fakegcs_testcontainer]
#[tokio::test]
async fn remove_crate() {
    let url = format!("http://localhost:{port}");
    let cratedata = Arc::new([0x00, 0x11, 0x22, 0x33, 0x44]);
    let test_storage = TestGCSStorage::from("test_delete_gcs", &url);
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

#[fakegcs_testcontainer]
#[tokio::test]
async fn docs_storage_delete_prefix_removes_only_matching_version() {
    // `localhost`, not `container.get_host()`: see the comment on `add_and_get_crate`.
    let url = format!("http://localhost:{port}");
    let docs = test_docs_storage("test_docs_delete_prefix_gcs", &url);

    let v1 = DocsStorage::file_key("my-crate", "1.0.0", "doc/my_crate/index.html");
    let v2 = DocsStorage::file_key("my-crate", "2.0.0", "doc/my_crate/index.html");
    docs.put(&v1, bytes::Bytes::from_static(b"a"))
        .await
        .unwrap();
    docs.put(&v2, bytes::Bytes::from_static(b"b"))
        .await
        .unwrap();

    docs.delete_prefix(&DocsStorage::version_prefix("my-crate", "1.0.0"))
        .await
        .unwrap();

    assert!(!docs.exists(&v1).await.unwrap());
    assert!(docs.exists(&v2).await.unwrap());
}

#[fakegcs_testcontainer]
#[tokio::test]
async fn get_with_meta_returns_bytes_etag_and_last_modified() {
    // `localhost`, not `container.get_host()`: see the comment on `add_and_get_crate`.
    let url = format!("http://localhost:{port}");
    let docs = test_docs_storage("test_docs_get_with_meta_gcs", &url);
    let key = DocsStorage::file_key("my-crate", "1.0.0", "doc/my_crate/index.html");
    docs.put(&key, bytes::Bytes::from_static(b"hello"))
        .await
        .unwrap();

    let object = docs.get_with_meta(&key).await.unwrap();

    assert_eq!(object.bytes, bytes::Bytes::from_static(b"hello"));
    assert!(object.e_tag.is_some());
    assert!(object.last_modified <= chrono::Utc::now());
}
