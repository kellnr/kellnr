use crate::session::{AdminUser, AnyUser};
use appstate::AppState;
use common::crate_data::CrateData;
use common::crate_overview::CrateOverview;
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::version::Version;
use db::error::DbError;
use db::DbProvider;
use index::rwindex::RwIndex;
use registry::kellnr_crate_storage::KellnrCrateStorage;
use reqwest::StatusCode;
use rocket::serde::json::Json;
use rocket::tokio::sync::{Mutex, RwLock};
use rocket::{catch, delete, get, http, post, Request, State};
use settings::Settings;
use tracing::error;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct KellnrVersion {
    pub version: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Pagination {
    crates: Vec<CrateOverview>,
    current_num: usize,
    total_num: usize,
}

pub async fn kellnr_version() -> axum::response::Json<KellnrVersion> {
    axum::response::Json(KellnrVersion {
        // Replaced automatically by the version from the build job,
        // if a new release is built.
        version: "0.0.0-debug".to_string(),
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CratesParams {
    page: Option<usize>,
    page_size: Option<usize>,
}

pub async fn crates(
    axum::extract::Query(params): axum::extract::Query<CratesParams>,
    axum::extract::State(state): AppState,
) -> axum::response::Json<Pagination> {
    let page_size = params.page_size.unwrap_or(10);
    let page = params.page;
    let crates = state.db.get_crate_overview_list().await.unwrap_or_default();
    let total = crates.len();

    let comp_start = |page: usize| {
        if page * page_size < crates.len() {
            page * page_size
        } else {
            0
        }
    };

    let comp_end = |start: usize| {
        if start + page_size < crates.len() {
            start + page_size
        } else {
            crates.len()
        }
    };

    let (end, crates) = match page {
        Some(p) => {
            let start = comp_start(p);
            let end = comp_end(start);
            (end, crates[start..end].to_vec())
        }
        None => (total, crates),
    };

    axum::Json(Pagination {
        crates,
        current_num: end,
        total_num: total,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SearchParams {
    name: OriginalName,
}

pub async fn search(
    axum::extract::Query(params): axum::extract::Query<SearchParams>,
    axum::extract::State(state): AppState,
) -> axum::response::Json<Pagination> {
    let crates = state
        .db
        .search_in_crate_name(&params.name)
        .await
        .unwrap_or_default();
    axum::Json(Pagination {
        current_num: crates.len(),
        total_num: crates.len(),
        crates,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CrateDataParams {
    name: OriginalName,
}

pub async fn crate_data(
    axum::extract::Query(params): axum::extract::Query<CrateDataParams>,
    axum::extract::State(state): AppState,
) -> Result<axum::response::Json<CrateData>, axum::http::StatusCode> {
    let index_name = NormalizedName::from(params.name);
    match state.db.get_crate_data(&index_name).await {
        Ok(cd) => Ok(axum::Json(cd)),
        Err(e) => match e {
            DbError::CrateNotFound(_) => Err(axum::http::StatusCode::NOT_FOUND),
            _ => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
        },
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CratesIoDataParams {
    name: OriginalName,
}

pub async fn cratesio_data(axum::extract::Query(params): axum::extract::Query<CratesIoDataParams>) -> 
Result<String, axum::http::StatusCode> {
    let url = format!("https://crates.io/api/v1/crates/{}", params.name);

    let client = reqwest::Client::new();
    let req = client
        .get(&url)
        .header("User-Agent", "kellnr")
        .header("Accept", "application/json");
    let resp = req.send().await;

    match resp {
        Ok(resp) => match resp.status() {
            StatusCode::OK => {
                let data = resp.text().await;
                match data {
                    Ok(data) => Ok(data),
                    Err(e) => {
                        error!("Failed to parse crates.io data: {}", e);
                        Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                    }
                }
            }
            StatusCode::NOT_FOUND => Err(axum::http::StatusCode::NOT_FOUND),
            _ => {
                error!("Failed to get crates.io data: {}", resp.status());
                Err(axum::http::StatusCode::NOT_FOUND)
            }
        },
        Err(e) => {
            error!("Failed to get crates.io data: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[delete("/crate?<name>&<version>")]
pub async fn delete(
    name: OriginalName,
    version: Version,
    _user: AdminUser,
    db: &State<Box<dyn DbProvider>>,
    idx: &State<Mutex<Box<dyn RwIndex>>>,
    storage: &State<RwLock<KellnrCrateStorage>>,
    settings: &State<Settings>,
) -> http::Status {
    if settings.git_index {
        if let Err(e) = idx.lock().await.delete(&name, &version).await {
            error!("Failed to delete crate from index: {}", e);
            return http::Status::InternalServerError;
        }
    }

    if let Err(e) = db.delete_crate(&name.to_normalized(), &version).await {
        error!("Failed to delete crate from database: {:?}", e);
        return http::Status::InternalServerError;
    }

    if let Err(e) = storage.write().await.delete(&name, &version).await {
        error!("Failed to delete crate from storage: {}", e);
        return http::Status::InternalServerError;
    }

    if let Err(e) = docs::delete(&name, &version, settings).await {
        error!("Failed to delete crate from docs: {}", e);
        return http::Status::InternalServerError;
    }

    http::Status::Ok
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Statistic {
    unique_crates: u32,
    crate_versions: u32,
    downloads: i64,
    top1: (String, u32),
    top2: (String, u32),
    top3: (String, u32),
}

pub async fn statistic(axum::extract::State(state): AppState) -> axum::response::Json<Statistic> {
    let unique_crates = state.db.get_total_unique_crates().await.unwrap_or_default();
    let crate_versions = state
        .db
        .get_total_crate_versions()
        .await
        .unwrap_or_default();
    let downloads = state.db.get_total_downloads().await.unwrap_or_default();
    let tops = state
        .db
        .get_top_crates_downloads(3)
        .await
        .unwrap_or_default();

    fn extract(tops: &[(String, u32)], i: usize) -> (String, u32) {
        if tops.len() > i {
            tops[i].clone()
        } else {
            (String::new(), 0)
        }
    }

    axum::Json(Statistic {
        unique_crates,
        crate_versions,
        downloads,
        top1: extract(&tops, 0),
        top2: extract(&tops, 1),
        top3: extract(&tops, 2),
    })
}

#[post("/build?<package>&<version>")]
pub async fn build_rustdoc(
    package: OriginalName,
    version: Version,
    db: &State<Box<dyn DbProvider>>,
    cs: &State<RwLock<KellnrCrateStorage>>,
    user: AnyUser,
) -> Result<rocket::http::Status, http::Status> {
    let normalized_name = NormalizedName::from(package);
    // Check if crate with the version exists.
    if let Some(id) = db
        .get_crate_id(&normalized_name)
        .await
        .map_err(|_| http::Status::InternalServerError)?
    {
        if !db
            .crate_version_exists(id, &version)
            .await
            .map_err(|_| http::Status::InternalServerError)?
        {
            return Err(http::Status::BadRequest);
        }
    } else {
        return Err(http::Status::BadRequest);
    }

    // Check if the current user is the owner of the crate
    use crate::session::Name;
    if !db
        .is_owner(&normalized_name, &user.name())
        .await
        .map_err(|_| http::Status::InternalServerError)?
        && !db
            .get_user(&user.name())
            .await
            .map_err(|_| http::Status::InternalServerError)?
            .is_admin
    {
        return Err(http::Status::Unauthorized);
    }

    // Add to build queue
    db.add_doc_queue(
        &normalized_name,
        &version,
        &cs.read()
            .await
            .create_rand_doc_queue_path()
            .await
            .map_err(|_| http::Status::InternalServerError)?,
    )
    .await
    .map_err(|_| http::Status::InternalServerError)?;

    Ok(http::Status::Ok)
}

#[delete("/index")]
pub async fn delete_cratesio_index(settings: &State<Settings>, _user: AdminUser) -> http::Status {
    index::cratesio_idx::remove_cratesio_index(settings.crates_io_index_path()).await;
    http::Status::Ok
}

/*
   Catch all 404 for SPA (Vue) as SPAs use their own routing in the browser. This is needed to
   support direct links to pages and page refreshes.
   See: https://router.vuejs.org/guide/essentials/history-mode.html
*/
#[catch(404)]
pub async fn not_found(_req: &Request<'_>) -> Option<rocket::fs::NamedFile> {
    let result = rocket::fs::NamedFile::open("static/index.html").await;
    result.ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::crate_data::{CrateRegistryDep, CrateVersionData};
    use db::error::DbError;
    use db::mock::MockDb;
    use db::User;
    use mockall::predicate::*;
    use rocket::http::{ContentType, Cookie, Header, Status};
    use rocket::local::asynchronous::Client;
    use rocket::{routes, Build, Rocket};
    use settings::constants;

    #[rocket::async_test]
    async fn build_rust_doc_crate_not_found() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(None));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), false)));
        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client
            .post("/build?package=foobar&version=1.0.0")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "cookie"))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", "token"));

        let result = req.dispatch().await;

        assert_eq!(rocket::http::Status::BadRequest, result.status());
    }

    #[rocket::async_test]
    async fn build_rust_doc_version_not_found() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(Some(1)));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), false)));
        mock_db
            .expect_crate_version_exists()
            .with(eq(1), eq("1.0.0"))
            .returning(move |_, _| Ok(false));
        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client
            .post("/build?package=foobar&version=1.0.0")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "cookie"))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", "token"));

        let result = req.dispatch().await;

        assert_eq!(rocket::http::Status::BadRequest, result.status());
    }

    #[rocket::async_test]
    async fn build_rust_doc_not_owner() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(Some(1)));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), false)));
        mock_db
            .expect_crate_version_exists()
            .with(eq(1), eq("1.0.0"))
            .returning(move |_, _| Ok(true));
        mock_db
            .expect_is_owner()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq("user"),
            )
            .returning(move |_, _| Ok(false));
        mock_db
            .expect_get_user()
            .with(eq("user"))
            .returning(move |_| {
                Ok(User {
                    id: 0,
                    name: "user".to_string(),
                    pwd: "".to_string(),
                    salt: "".to_string(),
                    is_admin: false,
                })
            });
        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client
            .post("/build?package=foobar&version=1.0.0")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "cookie"))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", "token"));

        let result = req.dispatch().await;

        assert_eq!(rocket::http::Status::Unauthorized, result.status());
    }

    #[rocket::async_test]
    async fn build_rust_doc_is_owner() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(Some(1)));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), false)));
        mock_db
            .expect_crate_version_exists()
            .with(eq(1), eq("1.0.0"))
            .returning(move |_, _| Ok(true));
        mock_db
            .expect_is_owner()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq("user"),
            )
            .returning(move |_, _| Ok(true));
        mock_db
            .expect_get_user()
            .with(eq("user"))
            .returning(move |_| {
                Ok(User {
                    id: 0,
                    name: "user".to_string(),
                    pwd: "".to_string(),
                    salt: "".to_string(),
                    is_admin: false,
                })
            });
        mock_db
            .expect_add_doc_queue()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq(Version::try_from("1.0.0").unwrap()),
                always(),
            )
            .times(1)
            .returning(move |_, _, _| Ok(()));

        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client
            .post("/build?package=foobar&version=1.0.0")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "cookie"))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", "token"));

        let result = req.dispatch().await;

        assert_eq!(rocket::http::Status::Ok, result.status());
    }

    #[rocket::async_test]
    async fn build_rust_doc_not_owner_but_admin() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(Some(1)));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), false)));
        mock_db
            .expect_crate_version_exists()
            .with(eq(1), eq("1.0.0"))
            .returning(move |_, _| Ok(true));
        mock_db
            .expect_is_owner()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq("user"),
            )
            .returning(move |_, _| Ok(false));
        mock_db
            .expect_get_user()
            .with(eq("user"))
            .returning(move |_| {
                Ok(User {
                    id: 0,
                    name: "user".to_string(),
                    pwd: "".to_string(),
                    salt: "".to_string(),
                    is_admin: true,
                })
            });
        mock_db
            .expect_add_doc_queue()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq(Version::try_from("1.0.0").unwrap()),
                always(),
            )
            .times(1)
            .returning(move |_, _, _| Ok(()));

        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client
            .post("/build?package=foobar&version=1.0.0")
            .private_cookie(Cookie::new(constants::COOKIE_SESSION_ID, "cookie"))
            .header(ContentType::JSON)
            .header(Header::new("Authorization", "token"));

        let result = req.dispatch().await;

        assert_eq!(rocket::http::Status::Ok, result.status());
    }

    #[rocket::async_test]
    async fn statistic_returns_sparse_statistics() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_total_unique_crates()
            .returning(move || Err(DbError::FailedToCountCrates));
        mock_db
            .expect_get_total_crate_versions()
            .returning(move || Err(DbError::FailedToCountCrateVersions));
        mock_db
            .expect_get_total_downloads()
            .returning(move || Err(DbError::FailedToCountTotalDownloads));
        mock_db
            .expect_get_top_crates_downloads()
            .with(eq(3))
            .returning(move |_| Ok(vec![("top1".to_string(), 1000)]));

        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/statistic");

        let result = req.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_stat = serde_json::from_str::<Statistic>(&result_msg).unwrap();

        let expect = Statistic {
            unique_crates: 0,
            crate_versions: 0,
            downloads: 0,
            top1: (String::from("top1"), 1000),
            top2: (String::new(), 0),
            top3: (String::new(), 0),
        };
        assert_eq!(expect, result_stat);
    }

    #[rocket::async_test]
    async fn statistic_returns_empty_statistics() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_total_unique_crates()
            .returning(move || Err(DbError::FailedToCountCrates));
        mock_db
            .expect_get_total_crate_versions()
            .returning(move || Err(DbError::FailedToCountCrateVersions));
        mock_db
            .expect_get_total_downloads()
            .returning(move || Err(DbError::FailedToCountTotalDownloads));
        mock_db
            .expect_get_top_crates_downloads()
            .with(eq(3))
            .returning(move |_| Err(DbError::FailedToCountTotalDownloads));

        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/statistic");

        let result = req.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_stat = serde_json::from_str::<Statistic>(&result_msg).unwrap();

        let expect = Statistic {
            unique_crates: 0,
            crate_versions: 0,
            downloads: 0,
            top1: (String::new(), 0),
            top2: (String::new(), 0),
            top3: (String::new(), 0),
        };
        assert_eq!(expect, result_stat);
    }

    #[rocket::async_test]
    async fn statistic_returns_crate_statistics() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_total_unique_crates()
            .returning(move || Ok(1000));
        mock_db
            .expect_get_total_crate_versions()
            .returning(move || Ok(10000));
        mock_db
            .expect_get_total_downloads()
            .returning(move || Ok(100000));
        mock_db
            .expect_get_top_crates_downloads()
            .with(eq(3))
            .returning(move |_| {
                Ok(vec![
                    ("top1".to_string(), 1000),
                    ("top2".to_string(), 500),
                    ("top3".to_string(), 100),
                ])
            });

        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/statistic");

        let result = req.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_stat = serde_json::from_str::<Statistic>(&result_msg).unwrap();

        let expect = Statistic {
            unique_crates: 1000,
            crate_versions: 10000,
            downloads: 100000,
            top1: ("top1".to_string(), 1000),
            top2: ("top2".to_string(), 500),
            top3: ("top3".to_string(), 100),
        };
        assert_eq!(expect, result_stat);
    }

    #[rocket::async_test]
    async fn kellnr_version_returns_version() {
        let settings = test_settings();
        let mock_db = MockDb::new();

        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/version");

        let result = req.dispatch().await;
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_version = serde_json::from_str::<KellnrVersion>(&result_msg).unwrap();

        assert_eq!("0.0.0-debug", result_version.version);
    }

    #[rocket::async_test]
    async fn search_not_hits_returns_nothing() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        mock_db
            .expect_search_in_crate_name()
            .with(eq("doesnotexist"))
            .returning(move |_name| Ok(vec![]));

        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/search?name=doesnotexist");

        let result = req.dispatch().await;
        let result_status = result.status();
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_crates = serde_json::from_str::<Pagination>(&result_msg).unwrap();

        assert_eq!(Status::Ok, result_status);
        assert_eq!(0, result_crates.crates.len());
        assert_eq!(0, result_crates.total_num);
        assert_eq!(0, result_crates.current_num);
    }

    #[rocket::async_test]
    async fn search_returns_only_searched_results() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        let test_crate_summary = CrateOverview {
            original_name: "hello".to_string(),
            max_version: "1.0.0".to_string(),
            last_updated: "12-10-2021 05:41:00".to_string(),
            total_downloads: 2,
            ..Default::default()
        };

        let tc = test_crate_summary.clone();
        mock_db
            .expect_search_in_crate_name()
            .with(eq("hello"))
            .returning(move |_| Ok(vec![tc.clone()]));

        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/search?name=hello");

        let result = req.dispatch().await;
        let result_status = result.status();
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_crates = serde_json::from_str::<Pagination>(&result_msg).unwrap();

        assert_eq!(Status::Ok, result_status);
        assert_eq!(1, result_crates.crates.len());
        assert_eq!(1, result_crates.total_num);
        assert_eq!(1, result_crates.current_num);
        assert_eq!(test_crate_summary, result_crates.crates[0]);
    }

    #[rocket::async_test]
    async fn crate_get_crate_information() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        let expected_crate_data = CrateData {
            name: "crate1".to_string(),
            owners: vec!["owner1".to_string(), "owner2".to_string()],
            max_version: "1.0.0".to_string(),
            total_downloads: 5,
            last_updated: "12-10-2021 05:41:00".to_string(),
            homepage: Some("homepage".to_string()),
            description: Some("description".to_string()),
            categories: vec!["cat1".to_string(), "cat2".to_string()],
            keywords: vec!["key1".to_string(), "key2".to_string()],
            authors: vec!["author1".to_string(), "author2".to_string()],
            repository: Some("repository".to_string()),
            versions: vec![CrateVersionData {
                version: "1.0.0".to_string(),
                created: "12-10-2021 05:41:00".to_string(),
                downloads: 5,
                readme: Some("readme".to_string()),
                license: Some("MIT".to_string()),
                license_file: Some("license".to_string()),
                documentation: Some("documentation".to_string()),
                dependencies: vec![CrateRegistryDep {
                    name: "dep1".to_string(),
                    description: Some("description".to_string()),
                    version_req: "1.0.0".to_string(),
                    features: None,
                    optional: false,
                    default_features: false,
                    target: Some("target".to_string()),
                    kind: Some("dev".to_string()),
                    registry: Some("registry".to_string()),
                    explicit_name_in_toml: None,
                }],
                checksum: "checksum".to_string(),
                features: Default::default(),
                yanked: false,
                links: Some("links".to_string()),
                v: 1,
            }],
        };

        let ecd = expected_crate_data.clone();
        mock_db
            .expect_get_crate_data()
            .returning(move |_| Ok(ecd.clone()));

        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/crate_data?name=c1&version=1.0.0");

        let result = req.dispatch().await;
        let result_status = result.status();
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_crate_data = serde_json::from_str::<CrateData>(&result_msg).unwrap();

        assert_eq!(Status::Ok, result_status);
        assert_eq!(expected_crate_data, result_crate_data);
    }

    #[rocket::async_test]
    async fn crates_get_page() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        let test_crate_overview = CrateOverview {
            original_name: "c1".to_string(),
            max_version: "1.0.0".to_string(),
            description: None,
            total_downloads: 2,
            last_updated: "12-10-2021 05:41:00".to_string(),
            documentation: None,
        };

        let test_crates = vec![
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
            test_crate_overview.clone(),
        ];

        let tc = test_crates.clone();
        mock_db
            .expect_get_crate_overview_list()
            .returning(move || Ok(tc.clone()));

        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/crates?page=1");

        let result = req.dispatch().await;
        let result_status = result.status();
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_pagination = serde_json::from_str::<Pagination>(&result_msg).unwrap();

        let expected = test_crates[0..10].to_vec();
        assert_eq!(Status::Ok, result_status);
        assert_eq!(24, result_pagination.total_num);
        assert_eq!(20, result_pagination.current_num);
        assert_eq!(10, result_pagination.crates.len());
        assert_eq!(expected, result_pagination.crates);
    }

    #[rocket::async_test]
    async fn crates_get_page_out_of_bounds() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        let test_crate_summary = CrateOverview {
            original_name: "c1".to_string(),
            max_version: "1.0.0".to_string(),
            last_updated: "12-10-2021 05:41:00".to_string(),
            total_downloads: 2,
            description: None,
            documentation: None,
        };

        let test_crates = vec![
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
            test_crate_summary.clone(),
        ];

        let tc = test_crates.clone();
        mock_db
            .expect_get_crate_overview_list()
            .returning(move || Ok(tc.clone()));

        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/crates?page=2");

        let result = req.dispatch().await;
        let result_status = result.status();
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_pagination = serde_json::from_str::<Pagination>(&result_msg).unwrap();

        let expected = test_crates[0..4].to_vec();
        assert_eq!(Status::Ok, result_status);
        assert_eq!(4, result_pagination.crates.len());
        assert_eq!(24, result_pagination.total_num);
        assert_eq!(24, result_pagination.current_num);
        assert_eq!(expected, result_pagination.crates);
    }

    #[rocket::async_test]
    async fn crates_get_all_crates() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        let expected_crate_overview = vec![
            CrateOverview {
                original_name: "c1".to_string(),
                max_version: "1.0.0".to_string(),
                last_updated: "12-11-2021 05:41:00".to_string(),
                total_downloads: 1,
                description: Some("Desc".to_string()),
                documentation: Some("Docs".to_string()),
            },
            CrateOverview {
                original_name: "c2".to_string(),
                max_version: "2.0.0".to_string(),
                last_updated: "12-12-2021 05:41:00".to_string(),
                total_downloads: 2,
                description: Some("Desc".to_string()),
                documentation: Some("Docs".to_string()),
            },
            CrateOverview {
                original_name: "c3".to_string(),
                max_version: "3.0.0".to_string(),
                last_updated: "12-09-2021 05:41:00".to_string(),
                total_downloads: 3,
                description: None,
                documentation: None,
            },
        ];

        let crate_overview = expected_crate_overview.clone();
        mock_db
            .expect_get_crate_overview_list()
            .returning(move || Ok(crate_overview.clone()));

        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");
        let req = client.get("/crates");

        let result = req.dispatch().await;
        let status = result.status();
        let result_msg = result.into_string().await.expect("Missing body message");
        let result_pagination = serde_json::from_str::<Pagination>(&result_msg).unwrap();

        assert_eq!(Status::Ok, status);
        assert_eq!(3, result_pagination.crates.len());
        assert_eq!(3, result_pagination.total_num);
        assert_eq!(3, result_pagination.current_num);
        assert_eq!(expected_crate_overview, result_pagination.crates);
    }

    #[rocket::async_test]
    async fn cratesio_data_returns_data() {
        let mock_db = MockDb::new();
        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");

        let req = client.get("/cratesio_data?name=quote");
        let result = req.dispatch().await;
        let status = result.status();
        let body = result.into_string().await.expect("Missing body message");

        assert_eq!(Status::Ok, status);
        assert!(body.contains("quote"));
    }

    #[rocket::async_test]
    async fn cratesio_data_not_found() {
        let mock_db = MockDb::new();
        let settings = test_settings();
        let rocket = create_rocket(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        );
        let client = Client::tracked(rocket)
            .await
            .expect("Unable to create rocket client");

        let req = client.get("/cratesio_data?name=thisdoesnotevenexist");
        let result = req.dispatch().await;

        assert_eq!(Status::NotFound, result.status());
    }

    fn test_settings() -> Settings {
        let settings = Settings::new().unwrap();
        Settings {
            data_dir: "/tmp/data".to_string(),
            ..settings
        }
    }

    fn create_rocket(
        mock_db: MockDb,
        crate_storage: KellnrCrateStorage,
        settings: Settings,
    ) -> Rocket<Build> {
        use rocket::config::{Config, SecretKey};
        let rocket_conf = Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        };

        rocket::custom(rocket_conf)
            .mount(
                "/",
                routes![
                    // search,
                    // crates,
                    // kellnr_version,
                    // statistic,
                    crate_data,
                    build_rustdoc,
                    cratesio_data,
                ],
            )
            .manage(RwLock::new(crate_storage))
            .manage(Box::new(mock_db) as Box<dyn DbProvider>)
            .manage(settings)
    }
}
