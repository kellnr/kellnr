use crate::kellnr_crate_storage::KellnrCrateStorage;
use crate::owner;
use crate::per_page;
use crate::pub_data::PubData;
use crate::pub_success::PubDataSuccess;
use crate::yank_success::YankSuccess;
use anyhow::Result;
use auth::auth_req_token::AuthReqToken;
use auth::token;
use chrono::Utc;
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::search_result;
use common::search_result::{Crate, SearchResult};
use common::version::Version;
use db::DbProvider;
use error::error::{ApiError, ApiResult};
use rocket::serde::json::Json;
use rocket::tokio::sync::RwLock;
use rocket::{delete, get, put, State};
use settings::Settings;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use tracing::warn;
use axum::response::Redirect;

type KellnrCrateStorageState = State<RwLock<KellnrCrateStorage>>;

#[allow(clippy::borrowed_box)]
pub async fn check_ownership(
    crate_name: &NormalizedName,
    token: &token::Token,
    db: &Box<dyn DbProvider>,
) -> Result<(), ApiError> {
    if token.is_admin || db.is_owner(crate_name, &token.user).await? {
        Ok(())
    } else {
        Err(ApiError::not_owner())
    }
}

pub fn crate_path(bin_path: &Path, name: &str, version: &str) -> PathBuf {
    bin_path.join(format!("{}-{}.crate", name, version))
}

pub async fn me() -> Redirect {
    Redirect::to("/login")
}

#[delete("/<crate_name>/owners", data = "<input>")]
pub async fn remove_owner(
    crate_name: OriginalName,
    input: owner::OwnerRequest,
    token: token::Token,
    db: &State<Box<dyn DbProvider>>,
) -> ApiResult<Json<owner::OwnerResponse>> {
    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, db).await?;

    for user in input.users.iter() {
        db.delete_owner(&crate_name, user).await?;
    }

    Ok(Json(owner::OwnerResponse::from(
        "Removed owners from crate.",
    )))
}

#[put("/<crate_name>/owners", data = "<input>")]
pub async fn add_owner(
    crate_name: OriginalName,
    input: owner::OwnerRequest,
    token: token::Token,
    db: &State<Box<dyn DbProvider>>,
) -> ApiResult<Json<owner::OwnerResponse>> {
    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, db).await?;
    for user in input.users.iter() {
        db.add_owner(&crate_name, user).await?;
    }

    Ok(Json(owner::OwnerResponse::from("Added owners to crate.")))
}

#[get("/<crate_name>/owners")]
pub async fn list_owners(
    crate_name: OriginalName,
    db: &State<Box<dyn DbProvider>>,
) -> ApiResult<Json<owner::OwnerList>> {
    let crate_name = crate_name.to_normalized();

    let owners: Vec<owner::Owner> = db
        .get_crate_owners(&crate_name)
        .await?
        .iter()
        .map(|u| owner::Owner {
            id: u.id,
            login: u.name.to_owned(),
            name: None,
        })
        .collect();

    Ok(Json(owner::OwnerList::from(owners)))
}

#[get("/?<q>&<per_page>")]
pub async fn search(
    q: OriginalName,
    per_page: per_page::PerPage,
    auth_req_token: AuthReqToken,
    db: &State<Box<dyn DbProvider>>,
) -> ApiResult<Json<search_result::SearchResult>> {
    _ = auth_req_token;
    let crates = db
        .search_in_crate_name(&q)
        .await?
        .into_iter()
        .map(|c| search_result::Crate {
            name: c.original_name,
            max_version: c.max_version,
            description: c
                .description
                .unwrap_or_else(|| "No description set".to_string()),
        })
        .take(per_page.0 as usize)
        .collect::<Vec<Crate>>();

    Ok(Json(SearchResult {
        meta: search_result::Meta {
            total: crates.len() as i32,
        },
        crates,
    }))
}

#[get("/<package>/<version>/download")]
pub async fn download(
    package: OriginalName,
    version: Version,
    auth_req_token: AuthReqToken,
    settings: &State<Settings>,
    db: &State<Box<dyn DbProvider>>,
    cs: &KellnrCrateStorageState,
) -> Option<Vec<u8>> {
    _ = auth_req_token;
    let file_path = crate_path(
        &settings.bin_path(),
        &package.to_string(),
        &version.to_string(),
    );

    if let Err(e) = db
        .increase_download_counter(&package.to_normalized(), &version)
        .await
    {
        warn!("Failed to increase download counter: {}", e);
    }

    cs.write().await.get_file(file_path).await
}

#[put("/new", data = "<input>")]
pub async fn publish(
    input: ApiResult<PubData>,
    db: &State<Box<dyn DbProvider>>,
    cs: &KellnrCrateStorageState,
    settings: &State<Settings>,
    token: token::Token,
) -> ApiResult<PubDataSuccess> {
    let pub_data = input?;
    let orig_name = OriginalName::try_from(&pub_data.metadata.name)?;
    let normalized_name = orig_name.to_normalized();

    // Check if user from token is an owner of the crate.
    // If not, he is not allowed push a new version.
    // Check if crate with same version already exists.
    let id = db.get_crate_id(&normalized_name).await?;
    if let Some(id) = id {
        check_ownership(&normalized_name, &token, db).await?;
        if db.crate_version_exists(id, &pub_data.metadata.vers).await? {
            return Err(ApiError::from(&format!(
                "Crate with version already exists: {}-{}",
                &pub_data.metadata.name, &pub_data.metadata.vers
            )));
        }
    }

    // Set SHA256 from crate file
    let version = Version::try_from(&pub_data.metadata.vers)?;
    let cksum = cs
        .read()
        .await
        .add_bin_package(&orig_name, &version, &pub_data.cratedata)
        .await?;

    let created = Utc::now();

    // Add crate to DB
    db.add_crate(&pub_data.metadata, &cksum, &created, &token.user)
        .await?;

    // Add crate to queue for doc extraction if there is no documentation value set already
    if settings.rustdoc_auto_gen && pub_data.metadata.documentation.is_none() {
        db.add_doc_queue(
            &normalized_name,
            &version,
            &cs.read().await.create_rand_doc_queue_path().await?,
        )
        .await?;
    }

    Ok(PubDataSuccess::new())
}

#[delete("/<crate_name>/<version>/yank")]
pub async fn yank(
    crate_name: OriginalName,
    version: Version,
    db: &State<Box<dyn DbProvider>>,
    token: token::Token,
) -> ApiResult<YankSuccess> {
    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, db).await?;

    db.yank_crate(&crate_name, &version).await?;

    Ok(YankSuccess::new())
}

#[put("/<crate_name>/<version>/unyank")]
pub async fn unyank(
    crate_name: OriginalName,
    version: Version,
    db: &State<Box<dyn DbProvider>>,
    token: token::Token,
) -> ApiResult<YankSuccess> {
    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, db).await?;

    db.unyank_crate(&crate_name, &version).await?;

    Ok(YankSuccess::new())
}

#[cfg(test)]
mod reg_api_tests {
    use super::*;
    use common::storage_provider::{mock::MockStorage, StorageProvider};
    use db::mock::MockDb;
    use db::{ConString, Database, SqliteConString};
    use mockall::predicate::*;
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    use rocket::http::{ContentType, Header, Status};
    use rocket::local::asynchronous::Client;
    use rocket::tokio::fs::File;
    use rocket::tokio::io::AsyncReadExt;
    use rocket::{async_test, routes, Build};
    use std::path::PathBuf;
    use std::{iter, path};

    const TOKEN: &str = "854DvwSlUwEHtIo3kWy6x7UCPKHfzCmy";

    #[async_test]
    async fn remove_owner_valid_owner() {
        let settings = get_settings();
        let kellnr = TestKellnr::new::<MockStorage>(settings).await;

        // Use valid crate publish data to test.
        let mut file = File::open("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut valid_pub_package = vec![];
        file.read_to_end(&mut valid_pub_package)
            .await
            .expect("Cannot read valid package file.");
        let request = kellnr
            .client
            .put("/api/v1/crates/new")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN))
            .body(valid_pub_package);
        request.dispatch().await;
        let del_owner = owner::OwnerRequest {
            users: vec![String::from("admin")],
        };

        let request = kellnr
            .client
            .delete("/api/v1/crates/test_lib/owners")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN))
            .body(serde_json::to_string(&del_owner).unwrap().as_bytes());
        let result = request.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing success message");

        assert_eq!(
            0,
            kellnr
                .db
                .get_crate_owners(&NormalizedName::from_unchecked("test_lib".to_string()))
                .await
                .unwrap()
                .len()
        );
        let owners = serde_json::from_str::<owner::OwnerResponse>(&result_msg).unwrap();
        assert!(owners.ok);
    }

    #[async_test]
    async fn add_owner_valid_owner() {
        let settings = get_settings();
        let kellnr = TestKellnr::new::<MockStorage>(settings).await;
        // Use valid crate publish data to test.
        let mut file = File::open("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut valid_pub_package = vec![];
        file.read_to_end(&mut valid_pub_package)
            .await
            .expect("Cannot read valid package file.");
        let request = kellnr
            .client
            .put("/api/v1/crates/new")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN))
            .body(valid_pub_package);
        request.dispatch().await;
        kellnr
            .db
            .add_user("user", "123", "123", false)
            .await
            .unwrap();
        let add_owner = owner::OwnerRequest {
            users: vec![String::from("user")],
        };

        let request = kellnr
            .client
            .put("/api/v1/crates/test_lib/owners")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN))
            .body(serde_json::to_string(&add_owner).unwrap().as_bytes());
        let result = request.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing success message");

        let owners = serde_json::from_str::<owner::OwnerResponse>(&result_msg).unwrap();
        assert!(owners.ok);
    }

    #[async_test]
    async fn list_owners_valid_owner() {
        let settings = get_settings();
        let kellnr = TestKellnr::new::<MockStorage>(settings).await;

        // Use valid crate publish data to test.
        let mut file = File::open("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut valid_pub_package = vec![];
        file.read_to_end(&mut valid_pub_package)
            .await
            .expect("Cannot read valid package file.");
        let request = kellnr
            .client
            .put("/api/v1/crates/new")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN))
            .body(valid_pub_package);
        request.dispatch().await;

        let request = kellnr
            .client
            .get("/api/v1/crates/test_lib/owners")
            .header(Header::new("Authorization", TOKEN));
        let result = request.dispatch().await;

        let result_msg = result.into_string().await.expect("Missing success message");

        let owners = serde_json::from_str::<owner::OwnerList>(&result_msg).unwrap();
        assert_eq!(1, owners.users.len());
        assert_eq!("admin", owners.users[0].login);
    }

    #[async_test]
    async fn publish_garbage() {
        let settings = get_settings();
        let kellnr = TestKellnr::new::<MockStorage>(settings).await;

        let garbage: [u8; 4] = [0x00, 0x11, 0x22, 0x33];
        let mut request = kellnr
            .client
            .put("/api/v1/crates/new")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN));

        request.set_body(garbage);
        let response = request.dispatch().await;
        let response_status = response.status();
        let error: ApiError =
            serde_json::from_str(&response.into_string().await.expect("Missing error message"))
                .expect("Cannot deserialize error message");

        assert_eq!(Status::Ok, response_status);
        assert_eq!(
            "ERROR: Invalid min. length. 4/10 bytes.",
            error.errors[0].detail
        );
    }

    #[async_test]
    async fn download_not_existing_package() {
        let settings = get_settings();
        let kellnr = TestKellnr::new::<MockStorage>(settings).await;
        let response = kellnr
            .client
            .get("/api/v1/crates/does_not_exist/0.1.0/download")
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::NotFound);
    }

    #[async_test]
    async fn download_invalid_package_name() {
        let settings = get_settings();
        let kellnr = TestKellnr::new::<MockStorage>(settings).await;
        let response = kellnr
            .client
            .get("/api/v1/crates/-invalid_name/0.1.0/download")
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::NotFound);
    }

    #[async_test]
    async fn download_not_existing_version() {
        let settings = get_settings();
        let kellnr = TestKellnr::new::<MockStorage>(settings).await;
        let response = kellnr
            .client
            .get("/api/v1/crates/test-lib/99.1.0/download")
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::NotFound);
    }

    #[async_test]
    async fn download_invalid_package_version() {
        let settings = get_settings();
        let kellnr = TestKellnr::new::<MockStorage>(settings).await;
        let response = kellnr
            .client
            .get("/api/v1/crates/invalid_version/0.a.0/download")
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::NotFound);
    }

    #[async_test]
    async fn search_verify_query_and_default() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name()
            .with(eq("foo"))
            .returning(|_| Ok(vec![]));

        let kellnr = test_client(Box::new(mock_db)).await;
        let request = kellnr.get("/api/v1/crates?q=foo");

        let result = request.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing success message");

        assert!(serde_json::from_str::<SearchResult>(&result_msg).is_ok());
    }

    #[async_test]
    async fn search_verify_per_page() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name()
            .with(eq("foo"))
            .returning(|_| Ok(vec![]));

        let kellnr = test_client(Box::new(mock_db)).await;
        let request = kellnr.get("/api/v1/crates?q=foo&per_page=20");

        let result = request.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing success message");

        assert!(serde_json::from_str::<SearchResult>(&result_msg).is_ok());
    }

    #[async_test]
    async fn search_verify_per_page_out_of_range() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let request = kellnr.client.get("/api/v1/crates?q=foo&per_page=200");

        let result = request.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing success message");

        assert!(serde_json::from_str::<search_result::SearchResult>(&result_msg).is_err());
    }

    #[async_test]
    async fn yank_success() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        // Use valid crate publish data to test.
        let mut file = File::open("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut valid_pub_package = vec![];
        file.read_to_end(&mut valid_pub_package)
            .await
            .expect("Cannot read valid package file.");
        let request = kellnr
            .client
            .put("/api/v1/crates/new")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN))
            .body(valid_pub_package);
        request.dispatch().await;

        let request = kellnr
            .client
            .delete("/api/v1/crates/test_lib/0.2.0/yank")
            .header(Header::new("Authorization", TOKEN));

        let result = request.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing success message");
        assert!(serde_json::from_str::<YankSuccess>(&result_msg).is_ok());
    }

    #[async_test]
    async fn yank_error() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let request = kellnr
            .client
            .delete("/api/v1/crates/test/0.1.0/yank")
            .header(Header::new("Authorization", TOKEN));

        let result = request.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing success message");

        assert!(serde_json::from_str::<ApiError>(&result_msg).is_ok());
    }

    #[async_test]
    async fn unyank_success() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        // Use valid crate publish data to test.
        let mut file = File::open("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut valid_pub_package = vec![];
        file.read_to_end(&mut valid_pub_package)
            .await
            .expect("Cannot read valid package file.");
        let request = kellnr
            .client
            .put("/api/v1/crates/new")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN))
            .body(valid_pub_package);
        request.dispatch().await;

        let request = kellnr
            .client
            .put("/api/v1/crates/test_lib/0.2.0/unyank")
            .header(Header::new("Authorization", TOKEN));

        let result = request.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing success message");
        assert!(serde_json::from_str::<YankSuccess>(&result_msg).is_ok());
    }

    #[async_test]
    async fn unyank_error() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let request = kellnr
            .client
            .put("/api/v1/crates/test/0.1.0/unyank")
            .header(Header::new("Authorization", TOKEN));

        let result = request.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing success message");

        assert!(serde_json::from_str::<ApiError>(&result_msg).is_ok());
    }

    #[async_test]
    async fn publish_package() {
        // Use valid crate publish data to test.
        let mut file = File::open("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut valid_pub_package = vec![];
        file.read_to_end(&mut valid_pub_package)
            .await
            .expect("Cannot read valid package file.");

        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;

        let request = kellnr
            .client
            .put("/api/v1/crates/new")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN))
            .body(valid_pub_package);

        // Get the empty success results message.
        let response = request.dispatch().await;
        let response_status = response.status();
        let result_msg = &mut response
            .into_string()
            .await
            .expect("Missing success message");
        let success: PubDataSuccess =
            serde_json::from_str(result_msg).expect("Cannot deserialize success message");

        assert_eq!(Status::Ok, response_status);
        assert_eq!(None, success.warnings);
        // As the success message is empty in the normal case, the deserialization works even
        // if an error message was returned. That's why we need to test for an error message, too.
        assert!(
            serde_json::from_str::<ApiError>(result_msg).is_err(),
            "An error message instead of a success message was returned"
        );
        assert_eq!(1, kellnr.db.get_crate_meta_list(1).await.unwrap().len());
        assert_eq!(
            "0.2.0",
            kellnr.db.get_crate_meta_list(1).await.unwrap()[0].version
        );
    }

    #[async_test]
    async fn publish_existing_package() {
        // Use valid crate publish data to test.
        let mut file = File::open("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut valid_pub_package = vec![];
        file.read_to_end(&mut valid_pub_package)
            .await
            .expect("Cannot read valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::new::<MockStorage>(settings).await;
        let request = kellnr
            .client
            .put("/api/v1/crates/new")
            .header(ContentType::JSON)
            .header(Header::new("Authorization", TOKEN))
            .body(&valid_pub_package);

        // Publish same package a second time.
        let _response = request.clone().dispatch().await;
        let response = request.dispatch().await;
        let response_status = response.status();

        let msg = response.into_string().await.expect("Missing error message");
        let error: ApiError = serde_json::from_str(&msg).expect("Cannot deserialize error message");

        assert_eq!(Status::Ok, response_status);
        assert_eq!(
            "ERROR: Crate with version already exists: test_lib-0.2.0",
            error.errors[0].detail
        );
    }

    struct TestKellnr {
        path: PathBuf,
        client: Client,
        db: Database,
    }

    fn get_settings() -> Settings {
        Settings {
            admin_pwd: "admin".to_string(),
            data_dir: "/tmp/".to_string() + &generate_rand_string(10),
            session_age_seconds: 10,
            ..Settings::new().unwrap()
        }
    }

    fn generate_rand_string(length: usize) -> String {
        let mut rng = thread_rng();
        iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(length)
            .collect::<String>()
    }

    impl TestKellnr {
        // why is T needed?
        #[allow(clippy::extra_unused_type_parameters)]
        async fn new<T: StorageProvider>(settings: Settings) -> Self {
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

        async fn fake(settings: Settings) -> Self {
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
        let con_string = ConString::Sqlite(SqliteConString::from(&settings));
        let db = Database::new(&con_string).await.unwrap();

        let db = Box::new(db) as Box<dyn DbProvider>;
        let cs = KellnrCrateStorage::new(&settings).await.unwrap();
        db.add_auth_token("test", TOKEN, "admin").await.unwrap();

        use rocket::config::{Config, SecretKey};
        let rocket_conf = Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        };

        rocket::custom(rocket_conf)
            .mount(
                "/api/v1/crates",
                routes![
                    download,
                    publish,
                    yank,
                    unyank,
                    search,
                    list_owners,
                    add_owner,
                    remove_owner,
                ],
            )
            .manage(settings)
            .manage(db)
            .manage(RwLock::new(cs))
    }

    async fn test_client(db: Box<dyn DbProvider>) -> Client {
        use rocket::config::{Config, SecretKey};
        let rocket_conf = Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        };

        let settings = Settings::new().unwrap();
        let rocket = rocket::custom(rocket_conf)
            .mount("/api/v1/crates", routes![search,])
            .manage(db)
            .manage(settings);

        Client::tracked(rocket)
            .await
            .expect("valid rocket instance")
    }
}
