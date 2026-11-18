//! Runs Kellnr with an object storage backend and checks that crates,
//! toolchains and docs are stored in their buckets and served from there.

use std::fs::File;
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use bytes::Bytes;
use kellnr_common::original_name::OriginalName;
use kellnr_common::publish_metadata::PublishMetadata;
use kellnr_common::version::Version;
use kellnr_settings::Settings;
use kellnr_settings::constants::COOKIE_SESSION_ID;
use kellnr_settings::gcs::Gcs;
use kellnr_settings::s3::S3;
use kellnr_storage::docs_storage::DocsName;
use kellnr_storage::storage_key::StorageKey;
use reqwest::{Client, Response, StatusCode, header};

const CRATES_BUCKET: &str = "kellnr-crates";
const TOOLCHAIN_BUCKET: &str = "kellnr-toolchains";
const DOCS_BUCKET: &str = "kellnr-docs";
const ADMIN_TOKEN: &str = "storage-backend-test-admin-token";

const CRATE_NAME: &str = "storage_test";
const CRATE_VERSION: &str = "0.1.0";
const CRATE_FILE: &[u8] = b"crate file of storage_test";
const DOCS_PAGE: &str = "doc/storage_test/index.html";
const DOCS_CONTENT: &[u8] = b"<html>docs of storage_test</html>";

const TOOLCHAIN_DATE: &str = "2024-01-15";
const TOOLCHAIN_TARGET: &str = "x86_64-unknown-linux-gnu";
const TOOLCHAIN_ARCHIVE: &str = "rust-1.0.0-test-x86_64-unknown-linux-gnu.tar.xz";
const TOOLCHAIN_COMPONENT: &str = "rustc-1.0.0-test-x86_64-unknown-linux-gnu.tar.xz";

/// Kellnr waits for the storage on startup and extracts toolchains in the
/// background; both take well under a second against the test containers.
const DEADLINE: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

enum Backend {
    S3 { endpoint: String },
    Gcs { endpoint: String },
}

impl Backend {
    /// The configuration of the backend as a Kellnr operator sets it.
    fn env(&self) -> Vec<(&'static str, String)> {
        match self {
            Backend::S3 { endpoint } => vec![
                ("KELLNR_S3__ENABLED", "true".into()),
                ("KELLNR_S3__ENDPOINT", endpoint.clone()),
                ("KELLNR_S3__ALLOW_HTTP", "true".into()),
                ("KELLNR_S3__ACCESS_KEY", "rustfsadmin".into()),
                ("KELLNR_S3__SECRET_KEY", "rustfsadmin".into()),
                ("KELLNR_S3__CRATES_BUCKET", CRATES_BUCKET.into()),
                ("KELLNR_S3__TOOLCHAIN_BUCKET", TOOLCHAIN_BUCKET.into()),
                ("KELLNR_S3__DOCS_BUCKET", DOCS_BUCKET.into()),
            ],
            Backend::Gcs { endpoint } => vec![
                ("KELLNR_GCS__ENABLED", "true".into()),
                ("KELLNR_GCS__ENDPOINT", endpoint.clone()),
                ("KELLNR_GCS__ALLOW_HTTP", "true".into()),
                ("KELLNR_GCS__SKIP_SIGNATURE", "true".into()),
                ("KELLNR_GCS__CRATES_BUCKET", CRATES_BUCKET.into()),
                ("KELLNR_GCS__TOOLCHAIN_BUCKET", TOOLCHAIN_BUCKET.into()),
                ("KELLNR_GCS__DOCS_BUCKET", DOCS_BUCKET.into()),
            ],
        }
    }

    /// Settings to read the buckets directly, independent of Kellnr.
    fn settings(&self) -> Settings {
        match self {
            Backend::S3 { endpoint } => Settings {
                s3: S3 {
                    enabled: true,
                    endpoint: Some(endpoint.clone()),
                    allow_http: true,
                    access_key: Some("rustfsadmin".into()),
                    secret_key: Some("rustfsadmin".into()),
                    ..S3::default()
                },
                ..kellnr_settings::test_settings()
            },
            Backend::Gcs { endpoint } => Settings {
                gcs: Gcs {
                    enabled: true,
                    endpoint: Some(endpoint.clone()),
                    allow_http: true,
                    skip_signature: true,
                    ..Gcs::default()
                },
                ..kellnr_settings::test_settings()
            },
        }
    }

    async fn assert_in_bucket(&self, bucket: &str, key: &StorageKey, expected: bool) {
        let storage = kellnr_storage::init_storage(bucket, &self.settings())
            .unwrap_or_else(|e| panic!("cannot open bucket {bucket}: {e}"));
        let exists = storage
            .exists(key)
            .await
            .unwrap_or_else(|e| panic!("cannot read {} in bucket {bucket}: {e}", key.as_str()));
        assert_eq!(exists, expected, "{} in bucket {bucket}", key.as_str());
    }
}

/// A Kellnr process with its own data directory and port, killed on drop.
struct Kellnr {
    child: Child,
    base_url: String,
    log_path: PathBuf,
    _data_dir: tempfile::TempDir,
}

impl Kellnr {
    async fn start(backend: &Backend) -> Self {
        let data_dir = tempfile::tempdir().unwrap();
        let log_path = data_dir.path().join("kellnr.log");
        let log = File::create(&log_path).unwrap();
        let port = free_port().to_string();

        let mut command = Command::new(env!("CARGO_BIN_EXE_kellnr"));
        for (key, _) in std::env::vars_os() {
            if key.as_encoded_bytes().starts_with(b"KELLNR_") {
                command.env_remove(key);
            }
        }
        let child = command
            .arg("start")
            .env("KELLNR_REGISTRY__DATA_DIR", data_dir.path())
            .env("KELLNR_LOCAL__IP", "127.0.0.1")
            .env("KELLNR_LOCAL__PORT", &port)
            .env("KELLNR_ORIGIN__HOSTNAME", "127.0.0.1")
            .env("KELLNR_ORIGIN__PORT", &port)
            .env("KELLNR_SETUP__ADMIN_TOKEN", ADMIN_TOKEN)
            .env("KELLNR_TOOLCHAIN__ENABLED", "true")
            .envs(backend.env())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .stdin(Stdio::null())
            .spawn()
            .expect("cannot start the kellnr binary");

        let mut kellnr = Kellnr {
            child,
            base_url: format!("http://127.0.0.1:{port}"),
            log_path,
            _data_dir: data_dir,
        };
        kellnr.wait_until_healthy().await;
        kellnr
    }

    async fn wait_until_healthy(&mut self) {
        let client = Client::new();
        let url = self.url("/api/v1/health");
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                panic!("kellnr exited with {status}:\n{}", self.log());
            }
            if client
                .get(&url)
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                return;
            }
            assert!(
                start.elapsed() < DEADLINE,
                "kellnr did not become healthy:\n{}",
                self.log()
            );
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn log(&self) -> String {
        std::fs::read_to_string(&self.log_path).unwrap_or_else(|e| format!("no log: {e}"))
    }
}

impl Drop for Kellnr {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

// The images of the storage tests. The container macros refer to them as
// `image`.
#[path = "../../storage/tests/gcs_image/mod.rs"]
mod fakegcs_image;
#[path = "../../storage/tests/image/mod.rs"]
mod rustfs_image;

mod s3 {
    use kellnr_rustfs_testcontainer::rustfs_testcontainer;

    use super::{Backend, run_scenario, rustfs_image as image};

    #[rustfs_testcontainer]
    #[tokio::test]
    async fn s3_backend_stores_and_serves_crates_toolchains_and_docs() {
        let host = container.get_host().await.unwrap();
        run_scenario(&Backend::S3 {
            endpoint: format!("http://{host}:{port}"),
        })
        .await;
    }
}

mod gcs {
    use kellnr_fakegcs_testcontainer::fakegcs_testcontainer;

    use super::{Backend, fakegcs_image as image, run_scenario};

    #[fakegcs_testcontainer]
    #[tokio::test]
    async fn gcs_backend_stores_and_serves_crates_toolchains_and_docs() {
        // `localhost`, as fake-gcs-server serves its object routes only for
        // its `-public-host` (see `gcs_image/mod.rs`).
        run_scenario(&Backend::Gcs {
            endpoint: format!("http://localhost:{port}"),
        })
        .await;
    }
}

/// Publishes a crate, its docs and a toolchain through Kellnr, checks that
/// they land in their buckets and are served from there, and that deleting
/// the toolchain target removes its archives from the bucket.
async fn run_scenario(backend: &Backend) {
    let kellnr = Kellnr::start(backend).await;
    let client = Client::new();
    check_crate_and_docs(backend, &kellnr, &client).await;
    check_toolchain_lifecycle(backend, &kellnr, &client).await;
}

async fn check_crate_and_docs(backend: &Backend, kellnr: &Kellnr, client: &Client) {
    publish_crate(client, kellnr).await;
    publish_docs(client, kellnr).await;
    let crate_key = StorageKey::crate_file(
        &OriginalName::try_from(CRATE_NAME).unwrap(),
        &Version::try_from(CRATE_VERSION).unwrap(),
    );
    backend
        .assert_in_bucket(CRATES_BUCKET, &crate_key, true)
        .await;
    let docs_key = StorageKey::docs_file(
        &DocsName::from(&OriginalName::try_from(CRATE_NAME).unwrap()),
        &Version::try_from(CRATE_VERSION).unwrap(),
        DOCS_PAGE,
    );
    backend.assert_in_bucket(DOCS_BUCKET, &docs_key, true).await;
    assert_eq!(
        get_ok(
            client,
            &kellnr.url(&format!(
                "/api/v1/crates/dl/{CRATE_NAME}/{CRATE_VERSION}/download"
            ))
        )
        .await,
        CRATE_FILE
    );
    assert_eq!(
        get_ok(
            client,
            &kellnr.url(&format!("/docs/{CRATE_NAME}/{CRATE_VERSION}/{DOCS_PAGE}"))
        )
        .await,
        DOCS_CONTENT
    );
}

async fn check_toolchain_lifecycle(backend: &Backend, kellnr: &Kellnr, client: &Client) {
    let session = login(client, kellnr).await;
    let fixture = toolchain_fixture();
    let archive = Bytes::from(
        std::fs::read(&fixture)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", fixture.display())),
    );
    upload_toolchain(client, kellnr, &session, archive.clone()).await;
    let archive_key = StorageKey::toolchain_file(TOOLCHAIN_DATE, TOOLCHAIN_ARCHIVE);
    let component_key = StorageKey::toolchain_file(TOOLCHAIN_DATE, TOOLCHAIN_COMPONENT);
    backend
        .assert_in_bucket(TOOLCHAIN_BUCKET, &archive_key, true)
        .await;
    backend
        .assert_in_bucket(TOOLCHAIN_BUCKET, &component_key, true)
        .await;
    let archive_url = kellnr.url(&format!(
        "/api/v1/toolchains/dist/{TOOLCHAIN_DATE}/{TOOLCHAIN_ARCHIVE}"
    ));
    let component_url = kellnr.url(&format!(
        "/api/v1/toolchains/dist/{TOOLCHAIN_DATE}/{TOOLCHAIN_COMPONENT}"
    ));
    assert_eq!(get_ok(client, &archive_url).await, archive);
    get_ok(client, &component_url).await;

    delete_toolchain_target(client, kellnr, &session).await;
    backend
        .assert_in_bucket(TOOLCHAIN_BUCKET, &archive_key, false)
        .await;
    backend
        .assert_in_bucket(TOOLCHAIN_BUCKET, &component_key, false)
        .await;
    for url in [archive_url, component_url] {
        let status = client.get(&url).send().await.unwrap().status();
        assert_eq!(status, StatusCode::NOT_FOUND, "{url}");
    }
}

/// The body of `cargo publish`: the metadata and the crate file, each
/// prefixed with its length.
async fn publish_crate(client: &Client, kellnr: &Kellnr) {
    let metadata =
        serde_json::to_vec(&PublishMetadata::minimal(CRATE_NAME, CRATE_VERSION)).unwrap();
    let mut body = Vec::new();
    for part in [metadata.as_slice(), CRATE_FILE] {
        body.extend_from_slice(&u32::try_from(part.len()).unwrap().to_le_bytes());
        body.extend_from_slice(part);
    }
    let response = client
        .put(kellnr.url("/api/v1/crates/new"))
        .header(header::AUTHORIZATION, ADMIN_TOKEN)
        .body(body)
        .send()
        .await
        .unwrap();
    expect_ok(response, "publish of the crate").await;
}

async fn publish_docs(client: &Client, kellnr: &Kellnr) {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file(DOCS_PAGE, zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(DOCS_CONTENT).unwrap();
    let body = zip.finish().unwrap().into_inner();
    let response = client
        .put(kellnr.url(&format!("/api/v1/docs/{CRATE_NAME}/{CRATE_VERSION}")))
        .header(header::AUTHORIZATION, ADMIN_TOKEN)
        .body(body)
        .send()
        .await
        .unwrap();
    expect_ok(response, "publish of the docs").await;
}

/// Logs in as the default admin and returns the session cookie, which the
/// toolchain API requires.
async fn login(client: &Client, kellnr: &Kellnr) -> String {
    let response = client
        .post(kellnr.url("/api/v1/auth/login"))
        .json(&serde_json::json!({ "user": "admin", "pwd": "admin" }))
        .send()
        .await
        .unwrap();
    let response = expect_ok(response, "login as admin").await;
    let prefix = format!("{COOKIE_SESSION_ID}=");
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| value.split(';').next())
        .find(|cookie| cookie.starts_with(&prefix))
        .expect("login returned no session cookie")
        .to_string()
}

/// Uploads the toolchain and waits until the stable manifest lists it, which
/// happens once its archive was split into components in the bucket.
async fn upload_toolchain(client: &Client, kellnr: &Kellnr, session: &str, archive: Bytes) {
    let response = client
        .put(kellnr.url(&format!(
            "/api/v1/toolchains?name=rust&version=1.0.0-test&target={TOOLCHAIN_TARGET}&date={TOOLCHAIN_DATE}&channel=stable"
        )))
        .header(header::COOKIE, session)
        .body(archive)
        .send()
        .await
        .unwrap();
    expect_ok(response, "upload of the toolchain").await;

    let manifest_url = kellnr.url("/api/v1/toolchains/dist/channel-rust-stable.toml");
    let start = Instant::now();
    loop {
        let response = client.get(&manifest_url).send().await.unwrap();
        if response.status().is_success()
            && response.text().await.unwrap().contains(TOOLCHAIN_COMPONENT)
        {
            return;
        }
        assert!(
            start.elapsed() < DEADLINE,
            "the stable manifest never listed {TOOLCHAIN_COMPONENT}:\n{}",
            kellnr.log()
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn delete_toolchain_target(client: &Client, kellnr: &Kellnr, session: &str) {
    let response = client
        .delete(kellnr.url(&format!(
            "/api/v1/toolchains/rust/1.0.0-test/targets/{TOOLCHAIN_TARGET}"
        )))
        .header(header::COOKIE, session)
        .send()
        .await
        .unwrap();
    expect_ok(response, "delete of the toolchain target").await;
}

fn toolchain_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/test-toolchain")
        .join(TOOLCHAIN_ARCHIVE)
}

async fn get_ok(client: &Client, url: &str) -> Vec<u8> {
    let response = client.get(url).send().await.unwrap();
    expect_ok(response, url)
        .await
        .bytes()
        .await
        .unwrap()
        .to_vec()
}

async fn expect_ok(response: Response, what: &str) -> Response {
    if response.status().is_success() {
        return response;
    }
    let status = response.status();
    panic!(
        "{what} failed with {status}: {}",
        response.text().await.unwrap_or_default()
    );
}
