use super::cratesio_crate_storage::CratesIoCrateStorage;
use crate::per_page;
use auth::auth_req_token::AuthReqToken;
use common::original_name::OriginalName;
use common::version::Version;
use error::error::{ApiError, ApiResult};
use reqwest::Url;
use rocket::log::private::{debug, trace};
use rocket::tokio::sync::RwLock;
use rocket::{get, State};
use settings::Settings;
use std::path::Path;
use tracing::error;

type CratesIoCrateStorageState = State<RwLock<CratesIoCrateStorage>>;

#[get("/?<q>&<per_page>")]
pub async fn search(
    q: OriginalName,
    per_page: per_page::PerPage,
    auth_req_token: AuthReqToken,
) -> ApiResult<String> {
    _ = auth_req_token;
    let url = match Url::parse(&format!(
        "https://crates.io/api/v1/crates?q={}&per_page={}",
        q, per_page.0
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

#[get("/<package>/<version>/download")]
pub async fn download(
    package: OriginalName,
    version: Version,
    auth_req_token: AuthReqToken,
    settings: &State<Settings>,
    crate_storage: &CratesIoCrateStorageState,
) -> Option<Vec<u8>> {
    _ = auth_req_token;
    // Return None if the feature is disabled
    match settings.crates_io_proxy {
        true => (),
        _ => return None,
    };

    let file_path = crate_storage
        .read()
        .await
        .crate_path(&package.to_string(), &version.to_string());

    trace!(
        "Downloading crate: {} ({}) from path {}",
        package,
        version,
        file_path.display()
    );

    if !Path::exists(&file_path) {
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
                        if !Path::exists(&file_path) {
                            if let Err(e) = crate_storage
                                .read()
                                .await
                                .add_bin_package(&package, &version, &crate_data)
                                .await
                            {
                                error!("Failed to save crate to disk: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to get crate data from response: {}", e);
                        return None;
                    }
                },
                // crates.io returned a 404 or another error -> Return NotFound
                false => return None,
            },
            Err(e) => {
                error!("Failed to download crate from crates.io: {}", e);
                return None;
            }
        }
    } else {
        trace!("Crate found in cache, skipping download");
    }

    crate_storage.write().await.get_file(file_path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::storage::Storage;
    use common::storage_provider::{mock::MockStorage, StorageProvider};
    use common::util::generate_rand_string;
    use db::{ConString, Database, SqliteConString};
    use index::cratesio_idx::CratesIoIdx;
    use index::rwindex::RoIndex;
    use rocket::http::Status;
    use rocket::local::asynchronous::Client;
    use rocket::tokio::sync::Mutex;
    use rocket::{async_test, routes, Build};
    use settings::Settings;
    use std::path;
    use std::path::PathBuf;

    #[async_test]
    async fn download_not_existing_package() {
        let settings = get_settings();
        let storage = Storage::new();
        let idx = CratesIoIdx::new(&settings, storage);
        let kellnr = TestKellnr::new::<MockStorage>(settings, Box::new(idx)).await;
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
        let storage = Storage::new();
        let idx = CratesIoIdx::new(&settings, storage);
        let kellnr = TestKellnr::new::<MockStorage>(settings, Box::new(idx)).await;
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
        let storage = Storage::new();
        let idx = CratesIoIdx::new(&settings, storage);
        let kellnr = TestKellnr::new::<MockStorage>(settings, Box::new(idx)).await;
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
        let storage = Storage::new();
        let idx = CratesIoIdx::new(&settings, storage);
        let kellnr = TestKellnr::new::<MockStorage>(settings, Box::new(idx)).await;
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
        let storage = Storage::new();
        let idx = CratesIoIdx::new(&settings, storage);
        let kellnr = TestKellnr::new::<MockStorage>(settings, Box::new(idx)).await;
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
        // why is T needed?
        #[allow(clippy::extra_unused_type_parameters)]
        async fn new<T: StorageProvider>(settings: Settings, idx: Box<dyn RoIndex>) -> Self {
            std::fs::create_dir_all(&settings.data_dir).unwrap();
            let con_string = ConString::Sqlite(SqliteConString::from(&settings));
            let db = Database::new(&con_string).await.unwrap();
            TestKellnr {
                path: path::PathBuf::from(&settings.data_dir),
                db,
                client: Client::tracked(test_rocket(settings, idx).await)
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

    async fn test_rocket(settings: Settings, idx: Box<dyn RoIndex>) -> rocket::Rocket<Build> {
        let cs = CratesIoCrateStorage::new(&settings).await.unwrap();

        use rocket::config::{Config, SecretKey};
        let rocket_conf = Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        };

        rocket::custom(rocket_conf)
            .mount("/api/v1/cratesio", routes![download, search,])
            .manage(settings)
            .manage(Mutex::new(idx))
            .manage(RwLock::new(cs))
    }
}
