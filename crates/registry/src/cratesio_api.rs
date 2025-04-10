use std::{error::Error, path::PathBuf, sync::Arc};

use crate::{registry_error::RegistryError, search_params::SearchParams};
use appstate::{CrateIoStorageState, CratesIoPrefetchSenderState, SettingsState};
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use common::{
    cratesio_prefetch_msg::{CratesioPrefetchMsg, DownloadData},
    original_name::OriginalName,
    version::Version,
};
use error::api_error::ApiResult;
use reqwest::{Client, ClientBuilder, Url};
use tracing::{error, trace, warn};

static CLIENT: std::sync::LazyLock<Client> = std::sync::LazyLock::new(|| {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("kellnr.io/kellnr"),
    );
    ClientBuilder::new()
        .gzip(true)
        .default_headers(headers)
        .build()
        .unwrap()
});

/// Middleware that checks if the crates.io proxy is enabled.
/// If not, a 404 is returned.
pub async fn cratesio_enabled(
    State(settings): SettingsState,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    match settings.proxy.enabled {
        true => Ok(next.run(request).await),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn search(params: SearchParams) -> ApiResult<String> {
    let url = Url::parse(&format!(
        "https://crates.io/api/v1/crates?q={}&per_page={}",
        params.q, params.per_page.0
    ))
    .map_err(RegistryError::UrlParseError)?;

    let response = CLIENT
        .get(url)
        .send()
        .await
        .map_err(RegistryError::RequestError)?;

    let body = response.text().await.map_err(RegistryError::RequestError)?;

    Ok(body)
}

pub async fn download(
    Path((package, version)): Path<(OriginalName, Version)>,
    State(crate_storage): CrateIoStorageState,
    State(sender): CratesIoPrefetchSenderState,
) -> Result<Vec<u8>, StatusCode> {
    let file_path = crate_storage.crate_path(&package.to_string(), &version.to_string());

    trace!(
        "Downloading crate: {} ({}) from path {}",
        package,
        version,
        PathBuf::from(file_path.clone()).display()
    );

    match crate_storage.get(file_path.as_str()).await {
        Some(file) => {
            let msg = DownloadData {
                name: package.into(),
                version,
            };
            if let Err(e) = sender.send(CratesioPrefetchMsg::IncDownloadCnt(msg)) {
                warn!("Failed to send IncDownloadCnt message: {}", e);
            }

            Ok(file)
        }
        None => {
            let target = format!(
                "https://static.crates.io/crates/{}/{}/download",
                package, version
            );

            let res = match CLIENT.get(target).send().await {
                Ok(resp) if resp.status() != 200 => Err(StatusCode::NOT_FOUND),
                Ok(resp) => Ok(resp),
                Err(e) => {
                    error!("Encountered error... {}", e);
                    Err(StatusCode::NOT_FOUND)
                }
            }?;

            let crate_data = res.bytes().await.map_err(log_return_error)?;
            let crate_data: Arc<[u8]> = Arc::from(crate_data.iter().as_slice());
            let _save = crate_storage
                .put(&package, &version, crate_data.clone())
                .await
                .map_err(|e| {
                    error!("Failed to save crate to disk: {}", e);
                    StatusCode::UNPROCESSABLE_ENTITY
                })?;

            crate_storage
                .get(file_path.as_str())
                .await
                .ok_or(StatusCode::NOT_FOUND)
        }
    }
}

fn log_return_error<E: Error>(e: E) -> StatusCode {
    error!("Failure while crate download...: {}", e);
    StatusCode::NOT_FOUND
}

#[cfg(test)]
mod tests {
    use super::*;
    use appstate::AppStateData;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::{Router, middleware};
    use common::util::generate_rand_string;
    use db::mock::MockDb;
    use http_body_util::BodyExt;
    use settings::Settings;
    use std::path;
    use std::path::PathBuf;
    use storage::cached_crate_storage::DynStorage;
    use storage::cratesio_crate_storage::CratesIoCrateStorage;
    use storage::fs_storage::FSStorage;
    use tower::ServiceExt;

    #[tokio::test]
    async fn download_not_existing_package() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
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
        let kellnr = TestKellnr::new(settings).await;
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
        let kellnr = TestKellnr::new(settings).await;
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
        let kellnr = TestKellnr::new(settings).await;
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

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_valid_package() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
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
        let kellnr = TestKellnr::new(settings).await;
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
            registry: settings::Registry {
                data_dir: "/tmp/".to_string() + &generate_rand_string(10),
                session_age_seconds: 10,
                ..settings::Registry::default()
            },
            proxy: settings::Proxy {
                enabled: true,
                ..settings::Proxy::default()
            },
            ..Settings::default()
        }
    }

    impl TestKellnr {
        async fn new(settings: Settings) -> Self {
            std::fs::create_dir_all(&settings.registry.data_dir).unwrap();
            TestKellnr {
                path: path::PathBuf::from(&settings.registry.data_dir),
                client: app(settings).await,
            }
        }
    }

    impl Drop for TestKellnr {
        fn drop(&mut self) {
            rm_rf::remove(&self.path).expect("Cannot remove TestKellnr")
        }
    }

    async fn app(settings: Settings) -> Router {
        let storage = Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage;
        let cs = CratesIoCrateStorage::new(&settings, storage).await.unwrap();
        let mut db = MockDb::new();
        db.expect_increase_cached_download_counter()
            .returning(|_, _| Ok(()));

        let state = AppStateData {
            settings: settings.into(),
            cratesio_storage: cs.into(),
            db: std::sync::Arc::<MockDb>::new(db),
            ..appstate::test_state().await
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
