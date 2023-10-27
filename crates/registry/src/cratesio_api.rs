use crate::per_page;
use appstate::{DbState, SettingsState, CrateIoStorageState};
use auth::auth_req_token::AuthReqToken;
use axum::{extract::{State, Query, Path}, Json, http::StatusCode};
use common::{original_name::OriginalName, search_result};
use common::version::Version;
use error::error::{ApiError, ApiResult};
use reqwest::Url;
use serde::Deserialize;
use settings::Settings;
use storage::cratesio_crate_storage::CratesIoCrateStorage;
use tracing::{error, trace, debug};

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: OriginalName,
    pub per_page: per_page::PerPage,
}

pub async fn search(
    auth_req_token: AuthReqToken,
    Query(params): Query<SearchParams>,
) -> ApiResult<String> {
    _ = auth_req_token;
    let url = match Url::parse(&format!(
        "https://crates.io/api/v1/crates?q={}&per_page={}",
        params.q, params.per_page.0
    )) {
        Ok(url) => url,
        Err(e) => {
            return Err(ApiError::from(&e.to_string()));
        }
    };

    let client = match reqwest::Client::builder().user_agent("kellnr").build() {
        Ok(client) => client,
        Err(e) => {
            return Err(ApiError::from(&e.to_string()));
        }
    };

    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(e) => {
            return Err(ApiError::from(&e.to_string()));
        }
    };

    let body = match response.text().await {
        Ok(body) => body,
        Err(e) => {
            return Err(ApiError::from(&e.to_string()));
        }
    };

    Ok(body)
}

// #[get("/<package>/<version>/download")]
pub async fn download(
    Path(package): Path<OriginalName>,
    Path(version): Path<Version>,
    auth_req_token: AuthReqToken,
    State(settings): SettingsState, 
    State(crate_storage):CrateIoStorageState
) -> Result<Vec<u8>, StatusCode> {
    _ = auth_req_token;
    // Return None if the feature is disabled
    match settings.crates_io_proxy {
        true => (),
        _ => return Err(StatusCode::NOT_FOUND),
    };

    let file_path = crate_storage
        .crate_path(&package.to_string(), &version.to_string());

    trace!(
        "Downloading crate: {} ({}) from path {}",
        package,
        version,
        file_path.display()
    );

    if !std::path::Path::exists(&file_path) {
        debug!("Crate not found on disk, downloading from crates.io");
        let target = format!(
            "https://crates.io/api/v1/crates/{}/{}/download",
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
        Some(file) => Ok(file),
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::util::generate_rand_string;
    use db::{ConString, Database, SqliteConString};
    use rocket::http::Status;
    use rocket::local::asynchronous::Client;
    use rocket::{async_test, routes, Build};
    use settings::Settings;
    use std::path;
    use std::path::PathBuf;

    #[async_test]
    async fn download_not_existing_package() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let response = kellnr
            .client
            .get("/api/v1/cratesio/does_not_exist/0.1.0/download")
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::NotFound);
    }

    #[async_test]
    async fn download_invalid_package_name() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let response = kellnr
            .client
            .get("/api/v1/cratesio/-invalid_name/0.1.0/download")
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::NotFound);
    }

    #[async_test]
    async fn download_not_existing_version() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let response = kellnr
            .client
            .get("/api/v1/cratesio/test-lib/99.1.0/download")
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::NotFound);
    }

    #[async_test]
    async fn download_invalid_package_version() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let response = kellnr
            .client
            .get("/api/v1/cratesio/invalid_version/0.a.0/download")
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::NotFound);
    }

    #[async_test]
    async fn download_valid_package() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let response = kellnr
            .client
            .get("/api/v1/cratesio/adler/1.0.2/download")
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);
        let body = response.into_bytes().await;
        assert_eq!(12778, body.unwrap().len());
    }

    struct TestKellnr {
        path: PathBuf,
        client: Client,
        #[allow(dead_code)]
        db: Database,
    }

    fn get_settings() -> Settings {
        Settings {
            admin_pwd: "admin".to_string(),
            data_dir: "/tmp/".to_string() + &generate_rand_string(10),
            session_age_seconds: 10,
            crates_io_proxy: true,
            ..Settings::new().unwrap()
        }
    }

    impl TestKellnr {
        async fn new(settings: Settings) -> Self {
            std::fs::create_dir_all(&settings.data_dir).unwrap();
            let con_string = ConString::Sqlite(SqliteConString::from(&settings));
            let db = Database::new(&con_string).await.unwrap();
            TestKellnr {
                path: path::PathBuf::from(&settings.data_dir),
                db,
                client: Client::tracked(test_rocket(settings).await)
                    .await
                    .expect("valid rocket instance"),
            }
        }
    }

    impl Drop for TestKellnr {
        fn drop(&mut self) {
            rm_rf::remove(&self.path).expect("Cannot remove TestKellnr")
        }
    }

    async fn test_rocket(settings: Settings) -> rocket::Rocket<Build> {
        let cs = CratesIoCrateStorage::new(&settings).await.unwrap();

        use rocket::config::{Config, SecretKey};
        let rocket_conf = Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        };

        rocket::custom(rocket_conf)
            .mount("/api/v1/cratesio", routes![download, search,])
            .manage(settings)
            .manage(RwLock::new(cs))
    }
}
