use appstate::{CrateIoStorageState, DbState, SettingsState};
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use common::{original_name::OriginalName, version::Version};
use error::api_error::ApiResult;
use reqwest::Url;
use tracing::{debug, error, trace};

use crate::{registry_error::RegistryError, search_params::SearchParams};

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

    let client = reqwest::Client::builder()
        .user_agent("kellnr")
        .build()
        .map_err(RegistryError::RequestError)?;

    let response = client
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
    State(db): DbState,
) -> Result<Vec<u8>, StatusCode> {
    let file_path = crate_storage.crate_path(&package.to_string(), &version.to_string());

    trace!(
        "Downloading crate: {} ({}) from path {}",
        package,
        version,
        file_path.display()
    );

    if !std::path::Path::exists(&file_path) {
        debug!("Crate not found on disk, downloading from crates.io");
        let target = format!(
            "https://static.crates.io/crates/{}/{}/download",
            package, version
        );
        match reqwest::get(target).await {
            Ok(response) => match response.status() == 200 {
                true => match response.bytes().await {
                    Ok(crate_data) => {
                        // Check again after the download, as another thread maybe
                        // added the crate already to disk and we can skip the step.
                        if !std::path::Path::exists(&file_path) {
                            if let Err(e) = crate_storage
                                .add_bin_package(&package, &version, &crate_data)
                                .await
                            {
                                error!("Failed to save crate to disk: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to get crate data from response: {}", e);
                        return Err(StatusCode::NOT_FOUND);
                    }
                },
                // crates.io returned a 404 or another error -> Return NotFound
                false => return Err(StatusCode::NOT_FOUND),
            },
            Err(e) => {
                error!("Failed to download crate from crates.io: {}", e);
                return Err(StatusCode::NOT_FOUND);
            }
        }
    } else {
        trace!("Crate found in cache, skipping download");
    }

    match crate_storage.get_file(file_path).await {
        Some(file) => {
            let normalized_name = package.to_normalized();
            db.increase_cached_download_counter(&normalized_name, &version)
                .await
                .unwrap_or_else(|e| error!("Failed to increase download counter: {}", e));
            Ok(file)
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use appstate::AppStateData;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::{middleware, Router};
    use common::util::generate_rand_string;
    use db::mock::MockDb;
    use http_body_util::BodyExt;
    use settings::Settings;
    use std::path;
    use std::path::PathBuf;
    use storage::cratesio_crate_storage::CratesIoCrateStorage;
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
        let cs = CratesIoCrateStorage::new(&settings).await.unwrap();
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
            .route("/:package/:version/download", get(download))
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                cratesio_enabled,
            ));

        Router::new()
            .nest("/api/v1/cratesio", routes)
            .with_state(state)
    }
}
