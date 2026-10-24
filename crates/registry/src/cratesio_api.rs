use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use bytes::Bytes;
use kellnr_appstate::{
    CrateIoStorageState, DbState, DownloadCounterState, ProxyClientState, SettingsState,
};
use kellnr_common::cratesio_downloader::{download_crate, has_checksum};
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::DbProvider;
use kellnr_error::api_error::ApiResult;
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
        (status = 404, description = "Crates.io proxy disabled")
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
        (status = 404, description = "Crate not found or proxy disabled"),
        (status = 422, description = "Failed to save crate"),
        (status = 502, description = "Crate from crates.io does not match the index checksum")
    ),
    security(("cargo_token" = []))
)]
pub async fn download(
    Path((name, version)): Path<(OriginalName, Version)>,
    State(crate_storage): CrateIoStorageState,
    State(download_counter): DownloadCounterState,
    State(settings): SettingsState,
    State(proxy_client): ProxyClientState,
    State(db): DbState,
) -> Result<Bytes, StatusCode> {
    trace!("Downloading crate: {name} ({version})");

    let file = if let Some(file) = crate_storage.get(&name, &version).await {
        file
    } else {
        let crate_data =
            download_crate(&proxy_client, &name, &version, &settings.proxy.url).await?;

        if is_verified_upstream_crate(&db, &name, &version, &crate_data).await? {
            cache_crate(&crate_storage, &name, &version, crate_data).await?
        } else {
            Bytes::copy_from_slice(&crate_data)
        }
    };

    // Count ALL downloads (both cache hits and upstream fetches)
    download_counter
        .increment_cached_and_maybe_flush(name.to_normalized(), version.clone())
        .await;

    Ok(file)
}

/// Check a crate downloaded from crates.io against the checksum of the cached
/// crates.io index. Returns `Ok(true)` if the crate matches and may be cached
/// and `Ok(false)` if no checksum is known yet. A mismatch is an error, as the
/// upstream or the connection to it is not trustworthy.
///
/// Without a known checksum, which happens if the index was not fetched through
/// Kellnr yet, the crate is served but not cached. Cargo verifies it against the
/// index it received, and the next download after the index is cached gets
/// verified and cached.
async fn is_verified_upstream_crate(
    db: &Arc<dyn DbProvider>,
    name: &OriginalName,
    version: &Version,
    crate_data: &[u8],
) -> Result<bool, StatusCode> {
    let cksum = db
        .get_cratesio_cksum(&name.to_normalized(), version)
        .await
        .map_err(|e| {
            error!("Could not get crates.io checksum for {name} ({version}): {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match cksum {
        Some(cksum) if has_checksum(crate_data, &cksum) => Ok(true),
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

async fn cache_crate(
    crate_storage: &CratesIoCrateStorage,
    name: &OriginalName,
    version: &Version,
    crate_data: Arc<[u8]>,
) -> Result<Bytes, StatusCode> {
    match crate_storage.put(name, version, crate_data).await {
        Ok(_) => crate_storage
            .get(name, version)
            .await
            .ok_or(StatusCode::NOT_FOUND),
        Err(StorageError::CrateExists(_, _)) => {
            trace!("Crate cache population raced for {name} ({version}); using the cached copy");
            crate_storage.get(name, version).await.ok_or_else(|| {
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

    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::{Router, middleware};
    use http_body_util::BodyExt;
    use kellnr_appstate::AppStateData;
    use kellnr_common::util::generate_rand_string;
    use kellnr_db::mock::MockDb;
    use kellnr_settings::Settings;
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn download_not_existing_package() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings);
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/cratesio/does_not_exist/0.1.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_invalid_package_name() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings);
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/cratesio/-invalid_name/0.1.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // An invalid crate name is rejected by the `OriginalName` extractor
        // before reaching the handler.
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn download_not_existing_version() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings);
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/cratesio/test-lib/99.1.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_invalid_package_version() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings);
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/cratesio/invalid_version/0.a.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Invalid semver is rejected during path deserialization (400 Bad Request)
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn download_valid_package() {
        let settings = get_settings();
        let mut db = MockDb::new();
        // Checksum of adler 1.0.2 from the crates.io index.
        db.expect_get_cratesio_cksum().returning(|_, _| {
            Ok(Some(
                "f26201604c87b1e01bd3d98f8d5d9a8fcbb815e8cedb41ffccbeb4bf593a35fe".to_string(),
            ))
        });
        let kellnr = TestKellnr::with_db(settings, db);
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/cratesio/adler/1.0.2/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
        let body = r.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(12778, body.len());
    }

    #[tokio::test]
    async fn cratesio_disabled_returns_404() {
        let mut settings = get_settings();
        settings.proxy.enabled = false;
        let kellnr = TestKellnr::new(settings);
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/cratesio/adler/1.0.2/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    const UPSTREAM_CRATE: &[u8] = b"upstream crate bytes";

    /// Serve `UPSTREAM_CRATE` for every download, like crates.io would.
    async fn fake_upstream() -> Url {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/{name}/{version}/download",
            get(|| async { UPSTREAM_CRATE }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Url::parse(&format!("http://{addr}/")).unwrap()
    }

    async fn download_from_upstream(cksum: Option<String>) -> (StatusCode, Bytes, bool) {
        let mut settings = get_settings();
        settings.proxy.url = fake_upstream().await;
        let mut db = MockDb::new();
        db.expect_get_cratesio_cksum()
            .returning(move |_, _| Ok(cksum.clone()));
        let kellnr = TestKellnr::with_db(settings, db);

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/cratesio/upstream/1.0.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = r.status();
        let body = r.into_body().collect().await.unwrap().to_bytes();
        let cached = kellnr
            .storage
            .exists(
                &OriginalName::try_from("upstream").unwrap(),
                &Version::try_from("1.0.0").unwrap(),
            )
            .await
            .unwrap();
        (status, body, cached)
    }

    #[tokio::test]
    async fn download_caches_crate_with_matching_checksum() {
        let cksum = sha256::digest(UPSTREAM_CRATE);

        let (status, body, cached) = download_from_upstream(Some(cksum)).await;

        assert_eq!(StatusCode::OK, status);
        assert_eq!(UPSTREAM_CRATE, &body[..]);
        assert!(cached);
    }

    #[tokio::test]
    async fn download_rejects_crate_with_wrong_checksum() {
        let cksum = sha256::digest(b"something else".as_slice());

        let (status, _, cached) = download_from_upstream(Some(cksum)).await;

        assert_eq!(StatusCode::BAD_GATEWAY, status);
        assert!(!cached);
    }

    #[tokio::test]
    async fn download_serves_but_does_not_cache_crate_without_known_checksum() {
        let (status, body, cached) = download_from_upstream(None).await;

        assert_eq!(StatusCode::OK, status);
        assert_eq!(UPSTREAM_CRATE, &body[..]);
        assert!(!cached);
    }

    struct TestKellnr {
        path: PathBuf,
        client: Router,
        storage: Arc<CratesIoCrateStorage>,
    }

    fn get_settings() -> Settings {
        Settings {
            registry: kellnr_settings::Registry {
                data_dir: "/tmp/".to_string() + &generate_rand_string(10),
                session_age_seconds: 10,
                ..kellnr_settings::Registry::default()
            },
            proxy: kellnr_settings::Proxy {
                enabled: true,
                ..kellnr_settings::Proxy::default()
            },
            ..Settings::default()
        }
    }

    impl TestKellnr {
        fn new(settings: Settings) -> Self {
            Self::with_db(settings, MockDb::new())
        }

        fn with_db(settings: Settings, db: MockDb) -> Self {
            std::fs::create_dir_all(&settings.registry.data_dir).unwrap();
            let storage =
                Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage;
            let storage = Arc::new(CratesIoCrateStorage::new(&settings, storage));
            TestKellnr {
                path: PathBuf::from(&settings.registry.data_dir),
                client: app(settings, storage.clone(), db),
                storage,
            }
        }
    }

    impl Drop for TestKellnr {
        fn drop(&mut self) {
            rm_rf::remove(&self.path).expect("Cannot remove TestKellnr");
        }
    }

    fn app(settings: Settings, storage: Arc<CratesIoCrateStorage>, db: MockDb) -> Router {
        let state = AppStateData {
            settings: settings.into(),
            cratesio_storage: storage,
            db: Arc::<MockDb>::new(db),
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
}
