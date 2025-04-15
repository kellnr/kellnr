use crate::pub_data::PubData;
use crate::pub_success::PubDataSuccess;
use crate::registry_error::RegistryError;
use crate::search_params::SearchParams;
use crate::yank_success::YankSuccess;
use crate::{crate_group, crate_user, crate_version};
use appstate::AppState;
use appstate::DbState;
use auth::token;
use axum::extract::Path;
use axum::extract::State;
use axum::response::Redirect;
use axum::Json;
use chrono::Utc;
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::search_result;
use common::search_result::{Crate, SearchResult};
use common::version::Version;
use db::DbProvider;
use error::api_error::{ApiError, ApiResult};
use std::convert::TryFrom;
use std::sync::Arc;
use tracing::warn;

pub async fn check_ownership(
    crate_name: &NormalizedName,
    token: &token::Token,
    db: &Arc<dyn DbProvider>,
) -> Result<(), ApiError> {
    if token.is_admin || db.is_owner(crate_name, &token.user).await? {
        Ok(())
    } else {
        Err(RegistryError::NotOwner.into())
    }
}

pub async fn check_can_modify(token: &token::Token) -> Result<(), ApiError> {
    if !token.is_admin && token.is_read_only {
        Err(RegistryError::ReadOnlyModify.into())
    } else {
        Ok(())
    }
}

pub async fn check_download_auth(
    crate_name: &NormalizedName,
    token: &token::OptionToken,
    db: &Arc<dyn DbProvider>,
) -> ApiResult<()> {
    if !db.is_download_restricted(crate_name).await? {
        return Ok(());
    }

    let token = match token {
        token::OptionToken::Some(token) => token,
        token::OptionToken::None => return Err(RegistryError::DownloadUnauthorized.into()),
    };

    if token.is_admin
        || db.is_crate_user(crate_name, &token.user).await?
        || db.is_crate_group_user(crate_name, &token.user).await?
        || db.is_owner(crate_name, &token.user).await?
    {
        Ok(())
    } else {
        Err(RegistryError::NotCrateUser.into())
    }
}

pub async fn me() -> Redirect {
    Redirect::to("/login")
}

pub async fn remove_owner(
    token: token::Token,
    State(db): DbState,
    Path(crate_name): Path<OriginalName>,
    Json(input): Json<crate_user::CrateUserRequest>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    // Check if user is read-only and can't remove an owner.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&token).await?;

    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    for user in input.users.iter() {
        db.delete_owner(&crate_name, user).await?;
    }

    Ok(Json(crate_user::CrateUserResponse::from(
        "Removed owners from crate.",
    )))
}

pub async fn remove_crate_user(
    token: token::Token,
    State(db): DbState,
    Path((crate_name, name)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    // Check if user is read-only and can't remove a crate user.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&token).await?;

    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    db.delete_crate_user(&crate_name, &name).await?;
    Ok(Json(crate_user::CrateUserResponse::from(
        "Removed users from crate.",
    )))
}

pub async fn remove_crate_group(
    token: token::Token,
    State(db): DbState,
    Path((crate_name, name)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_group::CrateGroupResponse>> {
    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    db.delete_crate_group(&crate_name, &name).await?;
    Ok(Json(crate_group::CrateGroupResponse::from(
        "Removed groups from crate.",
    )))
}

pub async fn add_owner(
    token: token::Token,
    State(db): DbState,
    Path(crate_name): Path<OriginalName>,
    Json(input): Json<crate_user::CrateUserRequest>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    // Check if user is read-only and can't add owners.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&token).await?;

    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    for user in input.users.iter() {
        db.add_owner(&crate_name, user).await?;
    }

    Ok(Json(crate_user::CrateUserResponse::from(
        "Added owners to crate.",
    )))
}

pub async fn list_owners(
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> ApiResult<Json<crate_user::CrateUserList>> {
    let crate_name = crate_name.to_normalized();

    let owners: Vec<crate_user::CrateUser> = db
        .get_crate_owners(&crate_name)
        .await?
        .iter()
        .map(|u| crate_user::CrateUser {
            id: u.id,
            login: u.name.to_owned(),
            name: None,
        })
        .collect();

    Ok(Json(crate_user::CrateUserList::from(owners)))
}

pub async fn add_crate_user(
    token: token::Token,
    State(db): DbState,
    Path((crate_name, name)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    // Check if user is read-only and can't add a crate user.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&token).await?;

    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    if !db.is_crate_user(&crate_name, &name).await? {
        db.add_crate_user(&crate_name, &name).await?;
    }

    Ok(Json(crate_user::CrateUserResponse::from(
        "Added users to crate.",
    )))
}

pub async fn list_crate_users(
    token: token::Token,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> ApiResult<Json<crate_user::CrateUserList>> {
    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    let users: Vec<crate_user::CrateUser> = db
        .get_crate_users(&crate_name)
        .await?
        .iter()
        .map(|u| crate_user::CrateUser {
            id: u.id,
            login: u.name.to_owned(),
            name: None,
        })
        .collect();

    Ok(Json(crate_user::CrateUserList::from(users)))
}

pub async fn add_crate_group(
    token: token::Token,
    State(db): DbState,
    Path((crate_name, name)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_group::CrateGroupResponse>> {
    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    if !db.is_crate_group(&crate_name, &name).await? {
        db.add_crate_group(&crate_name, &name).await?;
    }

    Ok(Json(crate_group::CrateGroupResponse::from(
        "Added groups to crate.",
    )))
}

pub async fn list_crate_groups(
    token: token::Token,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> ApiResult<Json<crate_group::CrateGroupList>> {
    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    let groups: Vec<crate_group::CrateGroup> = db
        .get_crate_groups(&crate_name)
        .await?
        .iter()
        .map(|u| crate_group::CrateGroup {
            id: u.id,
            name: u.name.to_owned(),
        })
        .collect();

    Ok(Json(crate_group::CrateGroupList::from(groups)))
}

pub async fn list_crate_versions(
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> ApiResult<Json<crate_version::CrateVersionList>> {
    let crate_name = crate_name.to_normalized();

    let versions: Vec<crate_version::CrateVersion> = db
        .get_crate_versions(&crate_name)
        .await?
        .into_iter()
        .map(|v| crate_version::CrateVersion {
            version: v.to_string(),
        })
        .collect();

    Ok(Json(crate_version::CrateVersionList::from(versions)))
}

pub async fn search(
    State(db): DbState,
    params: SearchParams,
) -> ApiResult<Json<search_result::SearchResult>> {
    let crates = db
        .search_in_crate_name(&params.q, false)
        .await?
        .into_iter()
        .map(|c| search_result::Crate {
            name: c.name,
            max_version: c.version,
            description: c
                .description
                .unwrap_or_else(|| "No description set".to_string()),
        })
        .take(params.per_page.0)
        .collect::<Vec<Crate>>();

    Ok(Json(SearchResult {
        meta: search_result::Meta {
            total: crates.len() as i32,
        },
        crates,
    }))
}

pub async fn download(
    State(state): AppState,
    token: token::OptionToken,
    Path((package, version)): Path<(OriginalName, Version)>,
) -> ApiResult<Vec<u8>> {
    let db = state.db;
    let cs = state.crate_storage;
    check_download_auth(&package.to_normalized(), &token, &db).await?;

    if let Err(e) = db
        .increase_download_counter(&package.to_normalized(), &version)
        .await
    {
        warn!("Failed to increase download counter: {}", e);
    }

    match cs.get(&package, &version).await {
        Some(file) => Ok(file),
        None => Err(RegistryError::CrateNotFound.into()),
    }
}

pub async fn publish(
    State(state): AppState,
    token: token::Token,
    pub_data: PubData,
) -> ApiResult<Json<PubDataSuccess>> {
    let db = state.db;
    let settings = state.settings;
    let cs = state.crate_storage;
    let orig_name = OriginalName::try_from(&pub_data.metadata.name)?;
    let normalized_name = orig_name.to_normalized();

    // Check if user is read-only and can't publish (upload) crates.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&token).await?;

    // Check if user from token is an owner of the crate.
    // If not, he is not allowed push a new version.
    // Check if crate with same version already exists.
    let id = db.get_crate_id(&normalized_name).await?;
    if let Some(id) = id {
        check_ownership(&normalized_name, &token, &db).await?;
        if db.crate_version_exists(id, &pub_data.metadata.vers).await? {
            return Err(
                RegistryError::CrateExists(pub_data.metadata.name, pub_data.metadata.vers).into(),
            );
        }
    }

    // Check if required crate fields aren't present in crate
    // Skip serializing if no fields to check
    if !settings.registry.required_crate_fields.is_empty() {
        let pkg_metadata = match serde_json::to_value(&pub_data.metadata)
            .map_err(RegistryError::InvalidMetadataString)?
        {
            serde_json::Value::Object(map) => map,
            _ => unreachable!(), // SAFETY: pub_data.metadata remains a struct
        };

        let mut missing_fields = Vec::new();

        for field in &settings.registry.required_crate_fields {
            // If field is null or not present, complain
            if let Some(serde_json::Value::Null) | None = pkg_metadata.get(field) {
                missing_fields.push(field.clone())
            }
        }

        if !missing_fields.is_empty() {
            return Err(RegistryError::MissingRequiredFields(
                pub_data.metadata.name,
                missing_fields,
                settings.registry.required_crate_fields.clone(),
            )
            .into());
        }
    }

    // Set SHA256 from crate file
    let version = Version::try_from(&pub_data.metadata.vers)?;
    let cksum = cs
        .put(&orig_name, &version, pub_data.cratedata.clone())
        .await?;

    let created = Utc::now();

    // Add crate to DB
    db.add_crate(&pub_data.metadata, &cksum, &created, &token.user)
        .await?;

    // Add crate to queue for doc extraction if there is no documentation value set already
    if settings.docs.enabled && pub_data.metadata.documentation.is_none() {
        db.add_doc_queue(
            &normalized_name,
            &version,
            &cs.create_rand_doc_queue_path().await?,
        )
        .await?;
    }

    Ok(Json(PubDataSuccess::new()))
}

pub async fn yank(
    Path((crate_name, version)): Path<(OriginalName, Version)>,
    token: token::Token,
    State(db): DbState,
) -> ApiResult<Json<YankSuccess>> {
    // Check if user is read-only and can't yank crates.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&token).await?;

    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    db.yank_crate(&crate_name, &version).await?;

    Ok(Json(YankSuccess::new()))
}

pub async fn unyank(
    Path((crate_name, version)): Path<(OriginalName, Version)>,
    token: token::Token,
    State(db): DbState,
) -> ApiResult<Json<YankSuccess>> {
    // Check if user is read-only and can't unyank crates.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&token).await?;

    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &token, &db).await?;

    db.unyank_crate(&crate_name, &version).await?;

    Ok(Json(YankSuccess::new()))
}

#[cfg(test)]
mod reg_api_tests {
    use super::*;
    use appstate::AppStateData;
    use axum::body::Body;
    use axum::http::Request;
    use axum::http::StatusCode;
    use axum::routing::{delete, get, put};
    use axum::Router;
    use db::mock::MockDb;
    use db::{test_utils, ConString, Database, SqliteConString};
    use error::api_error::ErrorDetails;
    use http_body_util::BodyExt;
    use hyper::header;
    use mockall::predicate::*;
    use rand::distr::Alphanumeric;
    use rand::rng;
    use rand::Rng;
    use settings::Settings;
    use std::path::PathBuf;
    use std::{iter, path};
    use storage::cached_crate_storage::DynStorage;
    use storage::fs_storage::FSStorage;
    use storage::kellnr_crate_storage::KellnrCrateStorage;
    use tokio::fs::read;
    use tower::ServiceExt;

    const TOKEN: &str = "854DvwSlUwEHtIo3kWy6x7UCPKHfzCmy";
    const RO_TOKEN: &str = "lJh6orU1Ye376ApXJR8I7V9gI3V6UZWU";
    const RO_ADMIN_TOKEN: &str = "GUOMPlZwN1kliXRW5wJ0ixh54NqYlE6X";

    #[tokio::test]
    async fn remove_owner_valid_owner() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // Use valid crate publish data to test.
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let del_owner = crate_user::CrateUserRequest {
            users: vec![String::from("admin")],
        };
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/owners")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(serde_json::to_string(&del_owner).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();

        assert_eq!(
            0,
            kellnr
                .db
                .get_crate_owners(&NormalizedName::from_unchecked("test_lib".to_string()))
                .await
                .unwrap()
                .len()
        );
        let owners = serde_json::from_slice::<crate_user::CrateUserResponse>(&result_msg).unwrap();
        assert!(owners.ok);
    }

    #[tokio::test]
    async fn add_owner_valid_owner() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        // Use valid crate publish data to test.
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();
        kellnr
            .db
            .add_user("user", "123", "123", false, false)
            .await
            .unwrap();
        let add_owner = crate_user::CrateUserRequest {
            users: vec![String::from("user")],
        };

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/owners")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(serde_json::to_string(&add_owner).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let owners = serde_json::from_slice::<crate_user::CrateUserResponse>(&result_msg).unwrap();
        assert!(owners.ok);
    }

    #[tokio::test]
    async fn list_owners_valid_owner() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // Use valid crate publish data to test.
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/crates/test_lib/owners")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();

        let owners = serde_json::from_slice::<crate_user::CrateUserList>(&result_msg).unwrap();
        assert_eq!(1, owners.users.len());
        assert_eq!("admin", owners.users[0].login);
    }

    #[tokio::test]
    async fn publish_garbage() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        let garbage = vec![0x00, 0x11, 0x22, 0x33];
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(garbage))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response_status = r.status();
        let error: ErrorDetails =
            serde_json::from_slice(r.into_body().collect().await.unwrap().to_bytes().as_ref())
                .expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            "ERROR: Invalid min. length 4/10 bytes",
            error.errors[0].detail
        );
    }

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
                Request::get("/api/v1/crates/test-lib/99.1.0/download")
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
                Request::get("/api/v1/crates/invalid_version/0.a.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn search_verify_query_and_default() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name()
            .with(eq("foo"), eq(false))
            .returning(|_, _| Ok(vec![]));

        let kellnr = app_search(Arc::new(mock_db)).await;
        let r = kellnr
            .oneshot(
                Request::get("/api/v1/crates?q=foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<SearchResult>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn search_verify_per_page() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name()
            .with(eq("foo"), eq(false))
            .returning(|_, _| Ok(vec![]));

        let kellnr = app_search(Arc::new(mock_db)).await;
        let r = kellnr
            .oneshot(
                Request::get("/api/v1/crates?q=foo&per_page=20")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<SearchResult>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn search_verify_per_page_out_of_range() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/crates?q=foo&per_page=200")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<search_result::SearchResult>(&result_msg).is_err());
    }

    #[tokio::test]
    async fn yank_success() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        // Use valid crate publish data to test.
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/0.2.0/yank")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<YankSuccess>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn yank_error() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test/0.1.0/yank")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<ErrorDetails>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn unyank_success() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        // Use valid crate publish data to test.
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/0.2.0/unyank")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<YankSuccess>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn unyank_error() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test/0.1.0/unyank")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<ErrorDetails>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn publish_package() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let success: PubDataSuccess =
            serde_json::from_slice(&result_msg).expect("Cannot deserialize success message");

        assert_eq!(StatusCode::OK, response_status);
        assert!(success.warnings.is_none());
        // As the success message is empty in the normal case, the deserialization works even
        // if an error message was returned. That's why we need to test for an error message, too.
        assert!(
            serde_json::from_slice::<ErrorDetails>(&result_msg).is_err(),
            "An error message instead of a success message was returned"
        );
        assert_eq!(
            1,
            test_utils::get_crate_meta_list(&kellnr.db, 1)
                .await
                .unwrap()
                .len()
        );
        assert_eq!(
            "0.2.0",
            test_utils::get_crate_meta_list(&kellnr.db, 1)
                .await
                .unwrap()[0]
                .version
        );
    }

    #[tokio::test]
    async fn try_publish_as_read_only_non_admin() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, RO_TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            "ERROR: Read-only users cannot modify the registry",
            error.errors[0].detail
        );
    }

    #[tokio::test]
    async fn try_publish_as_read_only_admin() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, RO_ADMIN_TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let success: PubDataSuccess =
            serde_json::from_slice(&result_msg).expect("Cannot deserialize success message");

        assert_eq!(StatusCode::OK, response_status);
        assert!(success.warnings.is_none());
        // As the success message is empty in the normal case, the deserialization works even
        // if an error message was returned. That's why we need to test for an error message, too.
        assert!(
            serde_json::from_slice::<ErrorDetails>(&result_msg).is_err(),
            "An error message instead of a success message was returned"
        );
        assert_eq!(
            1,
            test_utils::get_crate_meta_list(&kellnr.db, 1)
                .await
                .unwrap()
                .len()
        );
        assert_eq!(
            "0.2.0",
            test_utils::get_crate_meta_list(&kellnr.db, 1)
                .await
                .unwrap()[0]
                .version
        );
    }

    #[tokio::test]
    async fn publish_crate_with_missing_one_required_field() {
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut settings = get_settings();
        settings.registry.required_crate_fields = vec!["repository".to_string()];

        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            r#"ERROR: Required field(s) not defined for crate test_lib, missing: ["repository"], requires: ["repository"]"#,
            error.errors[0].detail
        );
    }

    #[tokio::test]
    async fn publish_crate_with_missing_multiple_required_fields() {
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut settings = get_settings();
        settings.registry.required_crate_fields =
            vec!["repository".to_string(), "license".to_string()];

        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            r#"ERROR: Required field(s) not defined for crate test_lib, missing: ["repository", "license"], requires: ["repository", "license"]"#,
            error.errors[0].detail
        );
    }

    // Missing some but not all required fields
    #[tokio::test]
    async fn publish_crate_with_some_required_fields() {
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut settings = get_settings();
        settings.registry.required_crate_fields =
            vec!["repository".to_string(), "authors".to_string()];

        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            r#"ERROR: Required field(s) not defined for crate test_lib, missing: ["repository"], requires: ["repository", "authors"]"#,
            error.errors[0].detail
        );
    }

    #[tokio::test]
    async fn publish_existing_package() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../test_data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Publish same package a second time.
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();
        let response_status = r.status();

        let msg = r.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            "ERROR: Crate with version already exists: test_lib-0.2.0",
            error.errors[0].detail
        );
    }

    struct TestKellnr {
        path: PathBuf,
        client: Router,
        db: Database,
    }

    fn get_settings() -> Settings {
        Settings {
            registry: settings::Registry {
                data_dir: "/tmp/".to_string() + &generate_rand_string(10),
                session_age_seconds: 10,
                ..settings::Registry::default()
            },
            ..Settings::default()
        }
    }

    fn generate_rand_string(length: usize) -> String {
        let mut rng = rng();
        iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(length)
            .collect::<String>()
    }

    impl TestKellnr {
        async fn new(settings: Settings) -> Self {
            std::fs::create_dir_all(&settings.registry.data_dir).unwrap();
            let con_string = ConString::Sqlite(SqliteConString::from(&settings));
            let db = Database::new(&con_string, 10).await.unwrap();
            TestKellnr {
                path: path::PathBuf::from(&settings.registry.data_dir),
                db,
                client: app(settings).await,
            }
        }

        async fn fake(settings: Settings) -> Self {
            std::fs::create_dir_all(&settings.registry.data_dir).unwrap();
            let con_string = ConString::Sqlite(SqliteConString::from(&settings));
            let db = Database::new(&con_string, 10).await.unwrap();

            TestKellnr {
                path: path::PathBuf::from(&settings.registry.data_dir),
                db,
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
        let con_string = ConString::Sqlite(SqliteConString::from(&settings));
        let db = Database::new(&con_string, 10).await.unwrap();
        let storage = Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
        let cs = KellnrCrateStorage::new(&settings, storage).await.unwrap();
        db.add_auth_token("test", TOKEN, "admin").await.unwrap();
        db.add_user("ro_dummy", "ro", "", false, true)
            .await
            .unwrap();
        db.add_auth_token("test ro", RO_TOKEN, "ro_dummy")
            .await
            .unwrap();
        db.add_user("ro_dummy_admin", "roa", "", true, true)
            .await
            .unwrap();
        db.add_auth_token("test admin ro", RO_ADMIN_TOKEN, "ro_dummy_admin")
            .await
            .unwrap();

        let state = AppStateData {
            db: Arc::new(db),
            settings: settings.into(),
            crate_storage: cs.into(),
            ..appstate::test_state().await
        };

        let routes = Router::new()
            .route("/{crate_name}/owners", delete(remove_owner))
            .route("/{crate_name}/owners", put(add_owner))
            .route("/{crate_name}/owners", get(list_owners))
            .route("/", get(search))
            .route("/{package}/{version}/download", get(download))
            .route("/new", put(publish))
            .route("/{crate_name}/{version}/yank", delete(yank))
            .route("/{crate_name}/{version}/unyank", put(unyank));

        Router::new()
            .nest("/api/v1/crates", routes)
            .with_state(state)
    }

    async fn app_search(db: Arc<dyn DbProvider>) -> Router {
        Router::new()
            .route("/api/v1/crates", get(search))
            .with_state(AppStateData {
                db,
                ..appstate::test_state().await
            })
    }
}
