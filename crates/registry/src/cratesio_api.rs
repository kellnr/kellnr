use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use kellnr_appstate::{AppState, ProxyClientState, SettingsState};
use kellnr_common::cratesio_downloader::{download_crate, has_checksum};
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_cratesio_sync::{CratesioSync, SyncSource};
use kellnr_db::DbProvider;
use kellnr_db::provider::CratesioVersionMeta;
use kellnr_error::api_error::ApiResult;
use kellnr_filter::{CrateInfo, KnownLicense};
use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
use kellnr_storage::storage_error::StorageError;
use reqwest::Url;
use tracing::{error, trace, warn};

use crate::registry_error::RegistryError;
use crate::search_params::SearchParams;

/// Middleware that checks if the crates.io proxy is enabled.
/// If not, a 404 is returned.
pub async fn cratesio_enabled(
    State(settings): SettingsState,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if settings.proxy.enabled {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Search crates.io
///
/// Proxies search requests to crates.io and returns the results.
#[utoipa::path(
    get,
    path = "/",
    tag = "cratesio",
    params(
        ("q" = String, Query, description = "Search query"),
        ("per_page" = Option<u32>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Search results from crates.io"),
        (status = 404, description = "Crates.io proxy disabled"),
        (status = 502, description = "Crates.io search failed")
    ),
    security(("cargo_token" = []))
)]
pub async fn search(
    State(proxy_client): ProxyClientState,
    State(settings): SettingsState,
    params: SearchParams,
) -> ApiResult<String> {
    let url = Url::parse_with_params(
        settings.proxy.api.as_str(),
        &[
            ("q", params.q.as_str()),
            ("per_page", &params.per_page.0.to_string()),
        ],
    )
    .map_err(RegistryError::UrlParseError)?;

    let response = proxy_client
        .get(url)
        .send()
        .await
        .map_err(RegistryError::RequestError)?;
    if !response.status().is_success() {
        return Err(RegistryError::UpstreamStatus(response.status()).into());
    }

    let body = response.text().await.map_err(RegistryError::RequestError)?;

    Ok(body)
}

/// Download a crate from crates.io
///
/// Downloads and caches a crate from crates.io. Returns the cached version
/// if available.
#[utoipa::path(
    get,
    path = "/dl/{package}/{version}/download",
    tag = "cratesio",
    params(
        ("package" = String, Path, description = "Package name"),
        ("version" = String, Path, description = "Package version")
    ),
    responses(
        (status = 200, description = "Crate archive", content_type = "application/octet-stream"),
        (status = 400, description = "Invalid package name or version"),
        (status = 403, description = "Download denied by a crates.io filter, e.g. the license filter"),
        (status = 404, description = "Crate not found or proxy disabled"),
        (status = 422, description = "Failed to save crate"),
        (status = 502, description = "Crate from crates.io does not match the index checksum")
    ),
    security(("cargo_token" = []))
)]
pub async fn download(
    State(state): AppState,
    Path((name, version)): Path<(OriginalName, Version)>,
) -> Result<Bytes, DownloadError> {
    trace!("Downloading crate: {name} ({version})");

    let canonical_name = name.to_canonical();
    let filters = &state.download_filters.cratesio;
    let cached = state
        .cratesio_storage
        .get(&name, &version)
        .await
        .map_err(|e| storage_read_failed(&name, &version, &e))?;

    // The license is only needed if filters are enabled, the checksum only to
    // verify a crate that is not cached yet. Cache hits without filters need
    // no database query.
    let known_license = if filters.is_empty() {
        None
    } else {
        filters.licenses().get(&canonical_name, &version)
    };
    let need_license = !filters.is_empty() && known_license.is_none();
    let meta = if cached.is_none() || need_license {
        synced_version_meta(
            &state.db,
            &state.cratesio_sync,
            &name,
            &version,
            need_license,
        )
        .await?
    } else {
        None
    };

    // Checked on cache hits too, as the policy may have changed since the
    // crate was cached.
    if !filters.is_empty() {
        let license = match known_license {
            Some(known) => known.into_license(),
            None => meta
                .as_ref()
                .and_then(|m| m.license.clone())
                .inspect(|license| {
                    // Only fetched licenses are remembered, a missing one is
                    // fetched again on the next download. A version without a
                    // license is stored as `UNKNOWN_LICENSE`, which is a license
                    // here.
                    filters.licenses().insert(
                        &canonical_name,
                        &version,
                        KnownLicense::Declared(license.clone()),
                    );
                }),
        };
        let krate = CrateInfo {
            name: &name,
            version: &version,
            license: license.as_deref(),
        };
        if let Some(message) = filters.check(&krate).deny_message(&krate) {
            warn!("{message}");
            return Err(DownloadError::Denied(message));
        }
    }

    let file = if let Some(file) = cached {
        file
    } else {
        let url = state.settings.proxy.download_url().map_err(|e| {
            error!("Invalid proxy.url: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let crate_data = download_crate(&state.proxy_client, &name, &version.to_string(), &url)
            .await
            .map_err(StatusCode::from)?;

        let cksum = meta.as_ref().map(|m| m.cksum.as_str());
        if is_verified_upstream_crate(&name, &version, cksum, &crate_data)? {
            cache_crate(&state.cratesio_storage, &name, &version, crate_data).await?
        } else {
            Bytes::copy_from_slice(&crate_data)
        }
    };

    // Count ALL downloads (both cache hits and upstream fetches)
    state
        .download_counter
        .increment_cached_and_maybe_flush(canonical_name, version.clone())
        .await;

    Ok(file)
}

/// Error of a download from the crates.io proxy.
pub enum DownloadError {
    Status(StatusCode),
    /// Denied by a filter, with the message for the client.
    Denied(String),
}

impl From<StatusCode> for DownloadError {
    fn from(status: StatusCode) -> Self {
        Self::Status(status)
    }
}

impl IntoResponse for DownloadError {
    fn into_response(self) -> Response {
        match self {
            Self::Status(status) => status.into_response(),
            Self::Denied(message) => (StatusCode::FORBIDDEN, message).into_response(),
        }
    }
}

/// Checksum and license of the version from the cached index. If the checksum
/// or a needed license is missing, the crate is synced from crates.io first,
/// or the running sync of it is awaited, so a cold download can be verified
/// and cached right away.
async fn synced_version_meta(
    db: &Arc<dyn DbProvider>,
    sync: &CratesioSync,
    name: &OriginalName,
    version: &Version,
    need_license: bool,
) -> Result<Option<CratesioVersionMeta>, StatusCode> {
    let get_meta = || async {
        db.get_cratesio_version_meta(&name.to_canonical(), version)
            .await
            .map_err(|e| {
                error!("Could not get crates.io metadata for {name} ({version}): {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })
    };

    let meta = get_meta().await?;
    if meta
        .as_ref()
        .is_some_and(|m| !need_license || m.license.is_some())
    {
        return Ok(meta);
    }
    trace!("Syncing {name} from crates.io, the metadata of {version} is incomplete");
    sync.sync(name, SyncSource::Fetch).await;
    get_meta().await
}

/// Check a crate downloaded from crates.io against the checksum of the cached
/// crates.io index. Returns `Ok(true)` if the crate matches and may be cached
/// and `Ok(false)` if no checksum is known. A mismatch is an error, as the
/// upstream or the connection to it is not trustworthy.
///
/// Without a known checksum, which happens if the index could not be synced
/// from crates.io, the crate is served but not cached. Cargo verifies it
/// against the index it received.
fn is_verified_upstream_crate(
    name: &OriginalName,
    version: &Version,
    cksum: Option<&str>,
    crate_data: &[u8],
) -> Result<bool, StatusCode> {
    match cksum {
        Some(cksum) if has_checksum(crate_data, cksum) => Ok(true),
        Some(_) => {
            error!("Checksum mismatch for crate {name} ({version}) downloaded from crates.io");
            Err(StatusCode::BAD_GATEWAY)
        }
        None => {
            warn!(
                "No crates.io index checksum known for {name} ({version}), serving the crate without caching it"
            );
            Ok(false)
        }
    }
}

fn storage_read_failed(name: &OriginalName, version: &Version, e: &StorageError) -> StatusCode {
    error!("Failed to read crate {name} ({version}) from the crates.io cache: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

async fn cache_crate(
    crate_storage: &CratesIoCrateStorage,
    name: &OriginalName,
    version: &Version,
    crate_data: Bytes,
) -> Result<Bytes, StatusCode> {
    match crate_storage.put(name, version, crate_data).await {
        Ok(_) => crate_storage
            .get(name, version)
            .await
            .map_err(|e| storage_read_failed(name, version, &e))?
            .ok_or(StatusCode::NOT_FOUND),
        Err(StorageError::CrateExists(_, _)) => {
            trace!("Crate cache population raced for {name} ({version}); using the cached copy");
            let cached = crate_storage
                .get(name, version)
                .await
                .map_err(|e| storage_read_failed(name, version, &e))?;
            cached.ok_or_else(|| {
                error!(
                    "Crate {name} ({version}) already existed after a cache race but could not be read afterward"
                );
                StatusCode::UNPROCESSABLE_ENTITY
            })
        }
        Err(error) => {
            error!("Failed to save crate to disk: {error}");
            Err(StatusCode::UNPROCESSABLE_ENTITY)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use axum::body::Body;
    use axum::extract::Path as AxumPath;
    use axum::http::{Request, header};
    use axum::routing::get;
    use axum::{Router, middleware};
    use http_body_util::BodyExt;
    use kellnr_appstate::AppStateData;
    use kellnr_common::util::generate_rand_string;
    use kellnr_db::{ConString, Database, SqliteConString};
    use kellnr_filter::DownloadFilters;
    use kellnr_settings::{CratesioFilter, LicenseUnknown, Settings};
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use tower::ServiceExt;

    use super::*;

    const UPSTREAM_CRATE: &[u8] = b"upstream crate bytes";

    async fn search_status(api: &str) -> StatusCode {
        let mut settings = Settings::default();
        settings.proxy.api = Url::parse(api).unwrap();
        let state = AppStateData {
            settings: settings.into(),
            proxy_client: reqwest::Client::new(),
            ..kellnr_appstate::test_state()
        };
        let app = Router::new().route("/", get(search)).with_state(state);

        app.oneshot(Request::get("/?q=foo").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn search_failing_upstream_is_bad_gateway() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/api",
            get(|| async { (StatusCode::TOO_MANY_REQUESTS, "slow down") }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        assert_eq!(
            StatusCode::BAD_GATEWAY,
            search_status(&format!("http://{addr}/api")).await
        );
    }

    #[tokio::test]
    async fn search_unreachable_upstream_is_bad_gateway() {
        // Nothing listens on port 1, so the connection is refused.
        assert_eq!(
            StatusCode::BAD_GATEWAY,
            search_status("http://127.0.0.1:1/api").await
        );
    }

    /// Versions of the crate "upstream" on the fake crates.io: the version,
    /// its license and whether the index checksum matches `UPSTREAM_CRATE`.
    const VERSIONS: &[(&str, Option<&str>, bool)] = &[
        ("1.0.0", Some("MIT OR Apache-2.0"), true),
        ("1.1.0", Some("GPL-3.0-only"), true),
        ("1.2.0", None, true),
        ("1.3.0", Some("MIT"), false),
    ];

    #[derive(Default)]
    struct Upstream {
        api_down: AtomicBool,
        api_calls: AtomicUsize,
        index_calls: AtomicUsize,
    }

    fn index_line(vers: &str, matches: bool) -> String {
        let cksum = if matches {
            sha256::digest(UPSTREAM_CRATE)
        } else {
            sha256::digest(b"something else".as_slice())
        };
        format!(
            r#"{{"name":"upstream","vers":"{vers}","deps":[],"cksum":"{cksum}","features":{{}},"yanked":false}}"#
        )
    }

    fn api_body() -> String {
        let versions: Vec<_> = VERSIONS
            .iter()
            .map(|(vers, license, _)| serde_json::json!({"num": vers, "license": license}))
            .collect();
        serde_json::json!({"crate": {"description": "upstream"}, "versions": versions}).to_string()
    }

    /// A fake crates.io with a sparse index, an API and downloads. Only the
    /// crate "upstream" exists in the index. Downloads exist for every crate,
    /// except for version 99.x and the crate `does_not_exist`.
    async fn fake_cratesio(upstream: Arc<Upstream>) -> kellnr_settings::Proxy {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let index_upstream = upstream.clone();
        let app = Router::new()
            .route(
                "/index/{a}/{b}/{name}",
                get(
                    move |AxumPath((_, _, name)): AxumPath<(String, String, String)>| {
                        let upstream = index_upstream.clone();
                        async move {
                            upstream.index_calls.fetch_add(1, Ordering::SeqCst);
                            // Slow, so concurrent downloads overlap
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            if name != "upstream" {
                                return StatusCode::NOT_FOUND.into_response();
                            }
                            let index: Vec<_> = VERSIONS
                                .iter()
                                .map(|(vers, _, matches)| index_line(vers, *matches))
                                .collect();
                            ([(header::ETAG, "etag")], index.join("\n")).into_response()
                        }
                    },
                ),
            )
            .route(
                "/api/{name}",
                get(move || {
                    let upstream = upstream.clone();
                    async move {
                        upstream.api_calls.fetch_add(1, Ordering::SeqCst);
                        if upstream.api_down.load(Ordering::SeqCst) {
                            StatusCode::INTERNAL_SERVER_ERROR.into_response()
                        } else {
                            api_body().into_response()
                        }
                    }
                }),
            )
            .route(
                "/dl/{name}/{version}/download",
                get(
                    |AxumPath((name, version)): AxumPath<(String, String)>| async move {
                        if name == "does_not_exist" || version.starts_with("99.") {
                            StatusCode::NOT_FOUND.into_response()
                        } else {
                            UPSTREAM_CRATE.into_response()
                        }
                    },
                ),
            );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        kellnr_settings::Proxy {
            enabled: true,
            index: Url::parse(&format!("http://{addr}/index/")).unwrap(),
            api: Url::parse(&format!("http://{addr}/api/")).unwrap(),
            url: Url::parse(&format!("http://{addr}/dl/")).unwrap(),
            ..kellnr_settings::Proxy::default()
        }
    }

    fn license_filter(unknown: LicenseUnknown) -> CratesioFilter {
        CratesioFilter {
            license_enabled: true,
            license_allowed: vec!["MIT".to_string()],
            license_unknown: unknown,
            license_exempt_crates: vec![],
        }
    }

    struct TestKellnr {
        path: PathBuf,
        client: Router,
        storage: Arc<CratesIoCrateStorage>,
        upstream: Arc<Upstream>,
    }

    impl TestKellnr {
        async fn new() -> Self {
            Self::with_filter(CratesioFilter::default()).await
        }

        async fn with_filter(filter: CratesioFilter) -> Self {
            let upstream = Arc::new(Upstream::default());
            let settings = Settings {
                registry: kellnr_settings::Registry {
                    data_dir: "/tmp/".to_string() + &generate_rand_string(10),
                    ..kellnr_settings::Registry::default()
                },
                proxy: fake_cratesio(upstream.clone()).await,
                cratesio_filter: filter,
                ..Settings::default()
            };
            std::fs::create_dir_all(&settings.registry.data_dir).unwrap();
            let storage =
                Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage;
            let storage = Arc::new(CratesIoCrateStorage::new(&settings, storage));
            let con_string = ConString::Sqlite(SqliteConString::from(&settings));
            let db: Arc<dyn DbProvider> = Arc::new(Database::new(&con_string, 10).await.unwrap());

            TestKellnr {
                path: PathBuf::from(&settings.registry.data_dir),
                client: app(settings, storage.clone(), db),
                storage,
                upstream,
            }
        }

        async fn download(&self, name: &str, version: &str) -> (StatusCode, Bytes) {
            let r = self
                .client
                .clone()
                .oneshot(
                    Request::get(format!("/api/v1/cratesio/{name}/{version}/download"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = r.status();
            (status, r.into_body().collect().await.unwrap().to_bytes())
        }

        async fn is_cached(&self, name: &str, version: &str) -> bool {
            self.storage
                .exists(
                    &OriginalName::try_from(name).unwrap(),
                    &Version::try_from(version).unwrap(),
                )
                .await
                .unwrap()
        }

        async fn cache(&self, name: &str, version: &str) {
            self.storage
                .put(
                    &OriginalName::try_from(name).unwrap(),
                    &Version::try_from(version).unwrap(),
                    Bytes::from_static(UPSTREAM_CRATE),
                )
                .await
                .unwrap();
        }

        fn api_calls(&self) -> usize {
            self.upstream.api_calls.load(Ordering::SeqCst)
        }
    }

    impl Drop for TestKellnr {
        fn drop(&mut self) {
            rm_rf::remove(&self.path).expect("Cannot remove TestKellnr");
        }
    }

    fn app(
        settings: Settings,
        storage: Arc<CratesIoCrateStorage>,
        db: Arc<dyn DbProvider>,
    ) -> Router {
        let download_filters = Arc::new(DownloadFilters::from_settings(&settings).unwrap());
        let proxy_client = kellnr_common::cratesio_downloader::CLIENT.clone();
        let state = AppStateData {
            cratesio_sync: CratesioSync::new(db.clone(), proxy_client.clone(), &settings.proxy),
            download_filters,
            proxy_client,
            settings: settings.into(),
            cratesio_storage: storage,
            db,
            ..kellnr_appstate::test_state()
        };

        let routes = Router::new()
            .route("/", get(search))
            .route("/{package}/{version}/download", get(download))
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                cratesio_enabled,
            ));

        Router::new()
            .nest("/api/v1/cratesio", routes)
            .with_state(state)
    }

    /// A Kellnr whose database is a mock, with "upstream" 1.0.0 in the cache.
    async fn kellnr_with_cached_crate(
        db: kellnr_db::mock::MockDb,
        filter: CratesioFilter,
    ) -> (Router, tempfile::TempDir) {
        let data_dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            registry: kellnr_settings::Registry {
                data_dir: data_dir.path().to_string_lossy().to_string(),
                ..kellnr_settings::Registry::default()
            },
            proxy: kellnr_settings::Proxy {
                enabled: true,
                ..kellnr_settings::Proxy::default()
            },
            cratesio_filter: filter,
            ..Settings::default()
        };
        let storage = Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage;
        let storage = Arc::new(CratesIoCrateStorage::new(&settings, storage));
        storage
            .put(
                &OriginalName::try_from("upstream").unwrap(),
                &Version::try_from("1.0.0").unwrap(),
                Bytes::from_static(UPSTREAM_CRATE),
            )
            .await
            .unwrap();
        (app(settings, storage, Arc::new(db)), data_dir)
    }

    async fn status_of(client: &Router, uri: &str) -> StatusCode {
        client
            .clone()
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn cache_hit_without_filters_needs_no_database() {
        // The mock panics on any database call
        let (client, _data_dir) =
            kellnr_with_cached_crate(kellnr_db::mock::MockDb::new(), CratesioFilter::default())
                .await;

        let status = status_of(&client, "/api/v1/cratesio/upstream/1.0.0/download").await;

        assert_eq!(StatusCode::OK, status);
    }

    #[tokio::test]
    async fn known_license_is_not_queried_again() {
        let mut db = kellnr_db::mock::MockDb::new();
        db.expect_get_cratesio_version_meta()
            .times(1)
            .returning(|_, _| {
                Ok(Some(CratesioVersionMeta {
                    cksum: sha256::digest(UPSTREAM_CRATE),
                    license: Some("MIT".to_string()),
                }))
            });
        let (client, _data_dir) =
            kellnr_with_cached_crate(db, license_filter(LicenseUnknown::Deny)).await;

        for _ in 0..3 {
            let status = status_of(&client, "/api/v1/cratesio/upstream/1.0.0/download").await;
            assert_eq!(StatusCode::OK, status);
        }
    }

    #[tokio::test]
    async fn download_not_existing_package() {
        let kellnr = TestKellnr::new().await;

        let (status, _) = kellnr.download("does_not_exist", "0.1.0").await;

        assert_eq!(StatusCode::NOT_FOUND, status);
    }

    #[tokio::test]
    async fn download_invalid_package_name() {
        let kellnr = TestKellnr::new().await;

        let (status, _) = kellnr.download("-invalid_name", "0.1.0").await;

        // An invalid crate name is rejected by the `OriginalName` extractor
        // before reaching the handler.
        assert_eq!(StatusCode::BAD_REQUEST, status);
    }

    #[tokio::test]
    async fn download_not_existing_version() {
        let kellnr = TestKellnr::new().await;

        let (status, _) = kellnr.download("upstream", "99.1.0").await;

        assert_eq!(StatusCode::NOT_FOUND, status);
    }

    #[tokio::test]
    async fn download_invalid_package_version() {
        let kellnr = TestKellnr::new().await;

        let (status, _) = kellnr.download("upstream", "0.a.0").await;

        // Invalid semver is rejected during path deserialization (400 Bad Request)
        assert_eq!(StatusCode::BAD_REQUEST, status);
    }

    #[tokio::test]
    async fn cratesio_disabled_returns_404() {
        let settings = Settings {
            registry: kellnr_settings::Registry {
                data_dir: "/tmp/".to_string() + &generate_rand_string(10),
                ..kellnr_settings::Registry::default()
            },
            ..Settings::default()
        };
        let data_dir = settings.registry.data_dir.clone();
        let storage = Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage;
        let storage = Arc::new(CratesIoCrateStorage::new(&settings, storage));
        let client = app(settings, storage, Arc::new(kellnr_db::mock::MockDb::new()));

        let r = client
            .oneshot(
                Request::get("/api/v1/cratesio/upstream/1.0.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, r.status());
        rm_rf::remove(data_dir).unwrap();
    }

    #[tokio::test]
    async fn cold_download_is_verified_and_cached_on_first_fetch() {
        // Regression test: the index was never fetched through Kellnr, so the
        // checksum is unknown until the download syncs the crate.
        let kellnr = TestKellnr::new().await;

        let (status, body) = kellnr.download("upstream", "1.0.0").await;

        assert_eq!(StatusCode::OK, status);
        assert_eq!(UPSTREAM_CRATE, &body[..]);
        assert!(kellnr.is_cached("upstream", "1.0.0").await);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn download_returns_500_if_the_cached_crate_cannot_be_read() {
        use std::os::unix::fs::PermissionsExt;

        let kellnr = TestKellnr::new().await;
        kellnr.cache("upstream", "1.0.0").await;
        let file = kellnr.path.join("cratesio/upstream-1.0.0.crate");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&file).is_ok() {
            // Running as root, the file stays readable.
            return;
        }

        let (status, _) = kellnr.download("upstream", "1.0.0").await;

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status);
    }

    #[tokio::test]
    async fn download_after_sync_does_not_sync_again() {
        let kellnr = TestKellnr::new().await;

        kellnr.download("upstream", "1.0.0").await;
        kellnr.download("upstream", "1.1.0").await;

        assert_eq!(1, kellnr.api_calls());
    }

    #[tokio::test]
    async fn concurrent_cold_downloads_sync_once() {
        let kellnr = TestKellnr::new().await;

        let (a, b) = tokio::join!(
            kellnr.download("upstream", "1.0.0"),
            kellnr.download("upstream", "1.1.0"),
        );

        assert_eq!(StatusCode::OK, a.0);
        assert_eq!(StatusCode::OK, b.0);
        assert_eq!(1, kellnr.api_calls());
        assert_eq!(1, kellnr.upstream.index_calls.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn download_rejects_crate_with_wrong_checksum() {
        let kellnr = TestKellnr::new().await;

        let (status, _) = kellnr.download("upstream", "1.3.0").await;

        assert_eq!(StatusCode::BAD_GATEWAY, status);
        assert!(!kellnr.is_cached("upstream", "1.3.0").await);
    }

    #[tokio::test]
    async fn download_serves_but_does_not_cache_crate_without_known_checksum() {
        // The index of the crate cannot be synced
        let kellnr = TestKellnr::new().await;

        let (status, body) = kellnr.download("noindex", "1.0.0").await;

        assert_eq!(StatusCode::OK, status);
        assert_eq!(UPSTREAM_CRATE, &body[..]);
        assert!(!kellnr.is_cached("noindex", "1.0.0").await);
    }

    #[tokio::test]
    async fn license_filter_allows_allowed_license() {
        let kellnr = TestKellnr::with_filter(license_filter(LicenseUnknown::Deny)).await;

        let (status, _) = kellnr.download("upstream", "1.0.0").await;

        assert_eq!(StatusCode::OK, status);
    }

    #[tokio::test]
    async fn license_filter_denies_on_cache_miss() {
        let kellnr = TestKellnr::with_filter(license_filter(LicenseUnknown::Deny)).await;

        let (status, body) = kellnr.download("upstream", "1.1.0").await;

        assert_eq!(StatusCode::FORBIDDEN, status);
        assert_eq!(
            "Download of upstream@1.1.0 denied by Kellnr license filter: license \"GPL-3.0-only\" is not allowed",
            String::from_utf8_lossy(&body)
        );
        assert!(!kellnr.is_cached("upstream", "1.1.0").await);
    }

    #[tokio::test]
    async fn license_filter_denies_on_cache_hit() {
        let kellnr = TestKellnr::with_filter(license_filter(LicenseUnknown::Deny)).await;
        kellnr.cache("upstream", "1.1.0").await;

        let (status, _) = kellnr.download("upstream", "1.1.0").await;

        assert_eq!(StatusCode::FORBIDDEN, status);
    }

    #[tokio::test]
    async fn license_filter_decides_on_declared_no_license() {
        let deny = TestKellnr::with_filter(license_filter(LicenseUnknown::Deny)).await;
        let allow = TestKellnr::with_filter(license_filter(LicenseUnknown::Allow)).await;

        assert_eq!(
            StatusCode::FORBIDDEN,
            deny.download("upstream", "1.2.0").await.0
        );
        assert_eq!(StatusCode::OK, allow.download("upstream", "1.2.0").await.0);
    }

    #[tokio::test]
    async fn license_unknown_decides_if_api_is_down_and_next_download_retries() {
        let deny = TestKellnr::with_filter(license_filter(LicenseUnknown::Deny)).await;
        deny.upstream.api_down.store(true, Ordering::SeqCst);
        let allow = TestKellnr::with_filter(license_filter(LicenseUnknown::Allow)).await;
        allow.upstream.api_down.store(true, Ordering::SeqCst);

        assert_eq!(
            StatusCode::FORBIDDEN,
            deny.download("upstream", "1.0.0").await.0
        );
        assert_eq!(StatusCode::OK, allow.download("upstream", "1.0.0").await.0);
        // The checksum was stored nonetheless
        assert!(allow.is_cached("upstream", "1.0.0").await);

        // The license is still unknown, so the next download syncs again and
        // gets it once the API is back
        deny.upstream.api_down.store(false, Ordering::SeqCst);
        assert_eq!(StatusCode::OK, deny.download("upstream", "1.0.0").await.0);
        // Three tries while the API was down, one after it is back
        assert_eq!(4, deny.api_calls());
    }

    #[tokio::test]
    async fn exempt_crate_is_downloaded_regardless_of_license() {
        let mut filter = license_filter(LicenseUnknown::Deny);
        filter.license_exempt_crates = vec!["upstream".to_string()];
        let kellnr = TestKellnr::with_filter(filter).await;

        let (status, _) = kellnr.download("upstream", "1.1.0").await;

        assert_eq!(StatusCode::OK, status);
    }
}
