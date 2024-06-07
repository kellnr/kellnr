use crate::error::RouteError;
use crate::session::MaybeUser;
use appstate::{AppState, DbState, SettingsState};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use common::crate_data::CrateData;
use common::crate_overview::CrateOverview;
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::version::Version;
use db::error::DbError;
use settings::Settings;
use tracing::error;

pub async fn settings(
    user: MaybeUser,
    State(settings): SettingsState,
) -> Result<Json<Settings>, RouteError> {
    user.assert_admin()?;
    let s: Settings = (*settings).to_owned();
    Ok(Json(s))
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct KellnrVersion {
    pub version: String,
}

pub async fn kellnr_version() -> Json<KellnrVersion> {
    Json(KellnrVersion {
        // Replaced automatically by the version from the build job,
        // if a new release is built.
        version: "0.0.0-debug".to_string(),
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CratesParams {
    page: Option<u64>,
    page_size: Option<u64>,
    cache: Option<bool>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Pagination {
    crates: Vec<CrateOverview>,
    page_size: u64,
    page: u64,
}

pub async fn crates(Query(params): Query<CratesParams>, State(db): DbState) -> Json<Pagination> {
    let page_size = params.page_size.unwrap_or(10);
    let page = params.page.unwrap_or(0);
    let cache = params.cache.unwrap_or(false);
    let crates = db
        .get_crate_overview_list(page_size, page_size * page, cache)
        .await
        .unwrap_or_default();

    Json(Pagination {
        crates,
        page_size,
        page,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SearchParams {
    name: OriginalName,
    cache: Option<bool>,
}

pub async fn search(Query(params): Query<SearchParams>, State(db): DbState) -> Json<Pagination> {
    let crates = db
        .search_in_crate_name(&params.name, params.cache.unwrap_or(false))
        .await
        .unwrap_or_default();
    Json(Pagination {
        page_size: crates.len() as u64,
        page: 0, // Return everything as one page
        crates,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CrateDataParams {
    name: OriginalName,
}

pub async fn crate_data(
    Query(params): Query<CrateDataParams>,
    State(db): DbState,
) -> Result<Json<CrateData>, StatusCode> {
    let index_name = NormalizedName::from(params.name);
    match db.get_crate_data(&index_name).await {
        Ok(cd) => Ok(Json(cd)),
        Err(e) => match e {
            DbError::CrateNotFound(_) => Err(StatusCode::NOT_FOUND),
            _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
        },
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CratesIoDataParams {
    name: OriginalName,
}

pub async fn cratesio_data(Query(params): Query<CratesIoDataParams>) -> Result<String, StatusCode> {
    let url = format!("https://crates.io/api/v1/crates/{}", params.name);

    let client = reqwest::Client::new();
    let req = client
        .get(&url)
        .header("User-Agent", "kellnr")
        .header("Accept", "application/json");
    let resp = req.send().await;

    match resp {
        Ok(resp) => match resp.status() {
            reqwest::StatusCode::OK => {
                let data = resp.text().await;
                match data {
                    Ok(data) => Ok(data),
                    Err(e) => {
                        error!("Failed to parse crates.io data: {}", e);
                        Err(StatusCode::INTERNAL_SERVER_ERROR)
                    }
                }
            }
            reqwest::StatusCode::NOT_FOUND => Err(StatusCode::NOT_FOUND),
            _ => {
                error!("Failed to get crates.io data: {}", resp.status());
                Err(StatusCode::NOT_FOUND)
            }
        },
        Err(e) => {
            error!("Failed to get crates.io data: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct DeleteCrateVersionParams {
    name: OriginalName,
    version: Version,
}

pub async fn delete_version(
    Query(params): Query<DeleteCrateVersionParams>,
    user: MaybeUser,
    State(state): AppState,
) -> Result<(), RouteError> {
    user.assert_admin()?;
    let version = params.version;
    let name = params.name;

    if let Err(e) = state.db.delete_crate(&name.to_normalized(), &version).await {
        error!("Failed to delete crate from database: {:?}", e);
        return Err(RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR));
    }

    if let Err(e) = state.crate_storage.delete(&name, &version).await {
        error!("Failed to delete crate from storage: {}", e);
        return Err(RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR));
    }

    if let Err(e) = docs::delete(&name, &version, &state.settings).await {
        error!("Failed to delete crate from docs: {}", e);
        return Err(RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR));
    }

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct DeleteCrateParams {
    name: OriginalName,
}

pub async fn delete_crate(
    Query(params): Query<DeleteCrateParams>,
    user: MaybeUser,
    State(state): AppState,
) -> Result<(), RouteError> {
    user.assert_admin()?;
    let name = params.name;

    let crate_meta = state.db.get_crate_meta_list(&name.to_normalized()).await?;

    for cm in crate_meta.iter() {
        let version = Version::from_unchecked_str(&cm.version);
        if let Err(e) = state.db.delete_crate(&name.to_normalized(), &version).await {
            error!("Failed to delete crate from database: {:?}", e);
            return Err(RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR));
        }

        if let Err(e) = state.crate_storage.delete(&name, &cm.version).await {
            error!("Failed to delete crate from storage: {}", e);
            return Err(RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR));
        }

        if let Err(e) = docs::delete(&name, &cm.version, &state.settings).await {
            error!("Failed to delete crate from docs: {}", e);
            return Err(RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR));
        }
    }

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Statistic {
    pub num_crates: u32,
    pub num_crate_versions: u32,
    pub num_crate_downloads: u64,
    pub num_proxy_crates: u64,
    pub num_proxy_crate_versions: u64,
    pub num_proxy_crate_downloads: u64,
    pub top_crates: TopCrates,
    pub last_updated_crate: Option<(OriginalName, Version)>,
    pub proxy_enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct TopCrates {
    pub first: (String, u64),
    pub second: (String, u64),
    pub third: (String, u64),
}

pub async fn statistic(State(db): DbState, State(settings): SettingsState) -> Json<Statistic> {
    let num_crates = db.get_total_unique_crates().await.unwrap_or_default();
    let num_crate_versions = db.get_total_crate_versions().await.unwrap_or_default();
    let num_crate_downloads = db.get_total_downloads().await.unwrap_or_default();
    let tops = db.get_top_crates_downloads(3).await.unwrap_or_default();
    let num_proxy_crates = db
        .get_total_unique_cached_crates()
        .await
        .unwrap_or_default();
    let num_proxy_crate_versions = db
        .get_total_cached_crate_versions()
        .await
        .unwrap_or_default();
    let num_proxy_crate_downloads = db.get_total_cached_downloads().await.unwrap_or_default();
    let last_updated_crate = db.get_last_updated_crate().await.unwrap_or_default();

    fn extract(tops: &[(String, u64)], i: usize) -> (String, u64) {
        if tops.len() > i {
            tops[i].clone()
        } else {
            (String::new(), 0)
        }
    }

    Json(Statistic {
        num_crates,
        num_crate_versions,
        num_crate_downloads,
        num_proxy_crates,
        num_proxy_crate_versions,
        num_proxy_crate_downloads,
        top_crates: TopCrates {
            first: extract(&tops, 0),
            second: extract(&tops, 1),
            third: extract(&tops, 2),
        },
        last_updated_crate,
        proxy_enabled: settings.proxy.enabled,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct BuildParams {
    package: OriginalName,
    version: Version,
}

pub async fn build_rustdoc(
    Query(params): Query<BuildParams>,
    State(state): AppState,
    user: MaybeUser,
) -> Result<(), StatusCode> {
    let normalized_name = NormalizedName::from(params.package);
    let db = state.db;
    let version = params.version;

    // Check if crate with the version exists.
    if let Some(id) = db
        .get_crate_id(&normalized_name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        if !db
            .crate_version_exists(id, &version)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            return Err(StatusCode::BAD_REQUEST);
        }
    } else {
        return Err(StatusCode::BAD_REQUEST);
    }

    // If the user is the owner of the crate or any admin user,
    // the build operation is allowed.
    let is_allowed = match user {
        MaybeUser::Normal(user) => db
            .is_owner(&normalized_name, &user)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?,
        MaybeUser::Admin(_) => true,
    };

    if !is_allowed {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Add to build queue
    db.add_doc_queue(
        &normalized_name,
        &version,
        &state
            .crate_storage
            .create_rand_doc_queue_path()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helper::encode_cookies;
    use appstate::AppStateData;
    use axum::body::Body;
    use axum::routing::{get, post};
    use axum::Router;
    use axum_extra::extract::cookie::Key;
    use common::crate_data::{CrateRegistryDep, CrateVersionData};
    use db::error::DbError;
    use db::mock::MockDb;
    use db::User;
    use http_body_util::BodyExt;
    use hyper::{header, Request};
    use mockall::predicate::*;
    use settings::Settings;
    use settings::{constants, Postgresql};
    use std::sync::Arc;
    use storage::kellnr_crate_storage::KellnrCrateStorage;
    use tower::ServiceExt;

    #[tokio::test]
    async fn settings_no_admin_returns_unauthorized() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .returning(|_| Ok(("admin".to_string(), true)));

        let settings = Settings::default();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(Request::get("/settings").body(Body::empty()).unwrap())
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn settings_returns_from_settings() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .returning(|_| Ok(("admin".to_string(), true)));

        let settings = Settings::default();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::get("/settings")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_state = serde_json::from_slice::<Settings>(&result_msg).unwrap();

        // Set the password to empty string because it is not serialized
        let tmp = Settings::default();
        let psq = Postgresql {
            pwd: String::default(),
            ..tmp.postgresql
        };
        let expected_state = Settings {
            postgresql: psq,
            ..tmp
        };

        assert_eq!(result_status, StatusCode::OK);
        assert_eq!(result_state, expected_state);
    }

    #[tokio::test]
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
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
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
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
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
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
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
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn build_rust_doc_not_owner_but_admin() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(Some(1)));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), true)));
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
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
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
        mock_db
            .expect_get_last_updated_crate()
            .returning(move || Ok(None));
        mock_db
            .expect_get_total_unique_cached_crates()
            .returning(move || Err(DbError::FailedToCountCrates));
        mock_db
            .expect_get_total_cached_crate_versions()
            .returning(move || Err(DbError::FailedToCountCrateVersions));
        mock_db
            .expect_get_total_cached_downloads()
            .returning(move || Err(DbError::FailedToCountTotalDownloads));

        let settings = test_settings();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(Request::get("/statistic").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_stat = serde_json::from_slice::<Statistic>(&result_msg).unwrap();

        let expect = Statistic {
            num_crates: 0,
            num_crate_versions: 0,
            num_crate_downloads: 0,
            num_proxy_crates: 0,
            num_proxy_crate_versions: 0,
            num_proxy_crate_downloads: 0,
            top_crates: TopCrates {
                first: (String::from("top1"), 1000),
                second: (String::new(), 0),
                third: (String::new(), 0),
            },
            last_updated_crate: None,
            proxy_enabled: false,
        };

        assert_eq!(expect, result_stat);
    }

    #[tokio::test]
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
        mock_db
            .expect_get_last_updated_crate()
            .returning(move || Ok(None));
        mock_db
            .expect_get_total_unique_cached_crates()
            .returning(move || Err(DbError::FailedToCountCrates));
        mock_db
            .expect_get_total_cached_crate_versions()
            .returning(move || Err(DbError::FailedToCountCrateVersions));
        mock_db
            .expect_get_total_cached_downloads()
            .returning(move || Err(DbError::FailedToCountTotalDownloads));

        let settings = test_settings();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(Request::get("/statistic").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_stat = serde_json::from_slice::<Statistic>(&result_msg).unwrap();

        let expect = Statistic {
            num_crates: 0,
            num_crate_versions: 0,
            num_crate_downloads: 0,
            num_proxy_crates: 0,
            num_proxy_crate_versions: 0,
            num_proxy_crate_downloads: 0,
            top_crates: TopCrates {
                first: (String::new(), 0),
                second: (String::new(), 0),
                third: (String::new(), 0),
            },
            last_updated_crate: None,
            proxy_enabled: false,
        };

        assert_eq!(expect, result_stat);
    }

    #[tokio::test]
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
        mock_db
            .expect_get_total_unique_cached_crates()
            .returning(move || Ok(9999));
        mock_db
            .expect_get_total_cached_crate_versions()
            .returning(move || Ok(99999));
        mock_db
            .expect_get_total_cached_downloads()
            .returning(move || Ok(999999));
        mock_db.expect_get_last_updated_crate().returning(move || {
            Ok(Some((
                OriginalName::from_unchecked_str("foobar".to_string()),
                Version::try_from("1.0.0").unwrap(),
            )))
        });

        let settings = test_settings();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(Request::get("/statistic").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_stat = serde_json::from_slice::<Statistic>(&result_msg).unwrap();

        let expect = Statistic {
            num_crates: 1000,
            num_crate_versions: 10000,
            num_crate_downloads: 100000,
            num_proxy_crates: 9999,
            num_proxy_crate_versions: 99999,
            num_proxy_crate_downloads: 999999,
            top_crates: TopCrates {
                first: (String::from("top1"), 1000),
                second: (String::from("top2"), 500),
                third: (String::from("top3"), 100),
            },
            last_updated_crate: Some((
                OriginalName::from_unchecked_str("foobar".to_string()),
                Version::try_from("1.0.0").unwrap(),
            )),
            proxy_enabled: false,
        };
        assert_eq!(expect, result_stat);
    }

    #[tokio::test]
    async fn kellnr_version_returns_version() {
        let settings = test_settings();
        let mock_db = MockDb::new();

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(Request::get("/version").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_version = serde_json::from_slice::<KellnrVersion>(&result_msg).unwrap();

        assert_eq!("0.0.0-debug", result_version.version);
    }

    #[tokio::test]
    async fn search_not_hits_returns_nothing() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        mock_db
            .expect_search_in_crate_name()
            .with(eq("doesnotexist"), eq(false))
            .returning(move |_name, _| Ok(vec![]));

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::get("/search?name=doesnotexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_crates = serde_json::from_slice::<Pagination>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(0, result_crates.crates.len());
        assert_eq!(0, result_crates.page);
        assert_eq!(0, result_crates.page_size);
    }

    #[tokio::test]
    async fn search_returns_only_searched_results() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        let test_crate_summary = CrateOverview {
            name: "hello".to_string(),
            version: "1.0.0".to_string(),
            date: "12-10-2021 05:41:00".to_string(),
            total_downloads: 2,
            ..Default::default()
        };

        let tc = test_crate_summary.clone();
        mock_db
            .expect_search_in_crate_name()
            .with(eq("hello"), eq(false))
            .returning(move |_, _| Ok(vec![tc.clone()]));

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::get("/search?name=hello")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_crates = serde_json::from_slice::<Pagination>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(1, result_crates.crates.len());
        assert_eq!(0, result_crates.page);
        assert_eq!(1, result_crates.page_size);
        assert_eq!(test_crate_summary, result_crates.crates[0]);
    }

    #[tokio::test]
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

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::get("/crate_data?name=crate1&version=1.0.0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_crate_data = serde_json::from_slice::<CrateData>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(expected_crate_data, result_crate_data);
    }

    #[tokio::test]
    async fn crates_get_page() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        let test_crate_overview = CrateOverview {
            name: "c1".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            total_downloads: 2,
            date: "12-10-2021 05:41:00".to_string(),
            documentation: None,
            is_cache: false,
        };

        let test_crates = std::iter::repeat_with(|| test_crate_overview.clone())
            .take(10)
            .collect::<Vec<_>>();

        let tc = test_crates.clone();
        mock_db
            .expect_get_crate_overview_list()
            .with(eq(10), eq(0), eq(false))
            .returning(move |_, _, _| Ok(tc.clone()));

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(Request::get("/crates?page=0").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_pagination = serde_json::from_slice::<Pagination>(&result_msg).unwrap();

        let expected = test_crates[0..10].to_vec();
        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(0, result_pagination.page);
        assert_eq!(10, result_pagination.page_size);
        assert_eq!(10, result_pagination.crates.len());
        assert_eq!(expected, result_pagination.crates);
    }

    #[tokio::test]
    async fn crates_get_all_crates() {
        let mut mock_db = MockDb::new();
        let settings = test_settings();

        let expected_crate_overview = vec![
            CrateOverview {
                name: "c1".to_string(),
                version: "1.0.0".to_string(),
                date: "12-11-2021 05:41:00".to_string(),
                total_downloads: 1,
                description: Some("Desc".to_string()),
                documentation: Some("Docs".to_string()),
                is_cache: true,
            },
            CrateOverview {
                name: "c2".to_string(),
                version: "2.0.0".to_string(),
                date: "12-12-2021 05:41:00".to_string(),
                total_downloads: 2,
                description: Some("Desc".to_string()),
                documentation: Some("Docs".to_string()),
                is_cache: true,
            },
            CrateOverview {
                name: "c3".to_string(),
                version: "3.0.0".to_string(),
                date: "12-09-2021 05:41:00".to_string(),
                total_downloads: 3,
                description: None,
                documentation: None,
                is_cache: true,
            },
        ];

        let crate_overview = expected_crate_overview.clone();
        mock_db
            .expect_get_crate_overview_list()
            .with(eq(10), eq(0), eq(false))
            .returning(move |_, _, _| Ok(crate_overview.clone()));

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(Request::get("/crates").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_pagination = serde_json::from_slice::<Pagination>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(3, result_pagination.crates.len());
        assert_eq!(0, result_pagination.page);
        assert_eq!(10, result_pagination.page_size);
        assert_eq!(expected_crate_overview, result_pagination.crates);
    }

    #[tokio::test]
    async fn cratesio_data_returns_data() {
        let mock_db = MockDb::new();
        let settings = test_settings();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::get("/cratesio_data?name=quote")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let body =
            String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
        assert!(body.contains("quote"));
        assert_eq!(StatusCode::OK, result_status);
    }

    #[tokio::test]
    async fn cratesio_data_not_found() {
        let mock_db = MockDb::new();
        let settings = test_settings();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings).await.unwrap(),
            settings,
        )
        .await
        .oneshot(
            Request::get("/cratesio_data?name=thisdoesnotevenexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    fn test_settings() -> Settings {
        Settings::default()
    }

    const TEST_KEY: &[u8] = &[1; 64];
    async fn app(mock_db: MockDb, crate_storage: KellnrCrateStorage, settings: Settings) -> Router {
        Router::new()
            .route("/search", get(search))
            .route("/crates", get(crates))
            .route("/crate_data", get(crate_data))
            .route("/version", get(kellnr_version))
            .route("/statistic", get(statistic))
            .route("/build", post(build_rustdoc))
            .route("/cratesio_data", get(cratesio_data))
            .route("/settings", get(crate::ui::settings))
            .with_state(AppStateData {
                db: Arc::new(mock_db),
                signing_key: Key::from(TEST_KEY),
                settings: Arc::new(settings),
                crate_storage: Arc::new(crate_storage),
                ..appstate::test_state().await
            })
    }
}
