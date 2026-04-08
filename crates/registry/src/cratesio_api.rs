use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use bytes::Bytes;
use kellnr_appstate::{CrateIoStorageState, DownloadCounterState, ProxyClientState, SettingsState};
use kellnr_common::cratesio_downloader::download_crate;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_error::api_error::ApiResult;
use kellnr_storage::storage_error::StorageError;
use reqwest::Url;
use tracing::{error, trace};

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
    params: SearchParams,
) -> ApiResult<String> {
    let url = Url::parse(&format!(
        "https://crates.io/api/v1/crates?q={}&per_page={}",
        params.q, params.per_page.0
    ))
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
        (status = 422, description = "Failed to save crate")
    ),
    security(("cargo_token" = []))
)]
pub async fn download(
    Path((name, version)): Path<(OriginalName, Version)>,
    State(crate_storage): CrateIoStorageState,
    State(download_counter): DownloadCounterState,
    State(settings): SettingsState,
    State(proxy_client): ProxyClientState,
) -> Result<Bytes, StatusCode> {
    trace!("Downloading crate: {name} ({version})");

    let file = if let Some(file) = crate_storage.get(&name, &version).await {
        file
    } else {
        let crate_data =
            download_crate(&proxy_client, &name, &version, &settings.proxy.url).await?;

        match crate_storage.put(&name, &version, crate_data.clone()).await {
            Ok(_) => crate_storage
                .get(&name, &version)
                .await
                .ok_or(StatusCode::NOT_FOUND)?,
            Err(StorageError::CrateExists(_, _)) => {
                trace!(
                    "Crate cache population raced for {name} ({version}); using the cached copy"
                );
                crate_storage.get(&name, &version).await.ok_or_else(|| {
                    error!(
                        "Crate {name} ({version}) already existed after a cache race but could not be read afterward"
                    );
                    StatusCode::UNPROCESSABLE_ENTITY
                })?
            }
            Err(error) => {
                error!("Failed to save crate to disk: {error}");
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
    };

    // Count ALL downloads (both cache hits and upstream fetches)
    download_counter
        .increment_cached_and_maybe_flush(name.to_normalized(), version.clone())
        .await;

    Ok(file)
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
    use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
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

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
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

    struct TestKellnr {
        path: PathBuf,
        client: Router,
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
            std::fs::create_dir_all(&settings.registry.data_dir).unwrap();
            TestKellnr {
                path: PathBuf::from(&settings.registry.data_dir),
                client: app(settings),
            }
        }
    }

    impl Drop for TestKellnr {
        fn drop(&mut self) {
            rm_rf::remove(&self.path).expect("Cannot remove TestKellnr");
        }
    }

    fn app(settings: Settings) -> Router {
        let storage = Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage;
        let cs = CratesIoCrateStorage::new(&settings, storage);
        let db = MockDb::new();

        let state = AppStateData {
            settings: settings.into(),
            cratesio_storage: cs.into(),
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
