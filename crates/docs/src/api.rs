use crate::{compute_doc_url, get_latest_version_with_doc};
use crate::doc_archive::DocArchive;
use crate::doc_queue_response::DocQueueResponse;
use crate::upload_response::DocUploadResponse;
use appstate::{AppState, DbState, SettingsState};
use auth::token::Token;
use axum::{
    extract::{Path, State}, response::Redirect, Json
};
use common::original_name::OriginalName;
use common::version::Version;
use error::error::{ApiError, ApiResult};
use registry::kellnr_api::check_ownership;

// #[get("/queue")]
pub async fn docs_in_queue(State(db): DbState) -> ApiResult<Json<DocQueueResponse>> {
    let doc = db.get_doc_queue().await?;
    Ok(Json(DocQueueResponse::from(doc)))
}

// #[get("/<package>/latest")]
pub async fn latest_docs(
    Path(package): Path<OriginalName>,
    State(settings): SettingsState,
    State(db): DbState,
) -> Redirect {
    let name = package.to_normalized();
    let opt_doc_version = get_latest_version_with_doc(&name, &settings);
    let res_db_version = db.get_max_version_from_name(&name).await;

    if let Some(doc_version) = opt_doc_version {
        if let Ok(db_version) = res_db_version {
            if doc_version == db_version {
                return Redirect::temporary(&compute_doc_url(&name, &db_version));
            }
        }
    }

    Redirect::temporary("/")
}

// #[put("/<package>/<version>", data = "<docs>")]
pub async fn publish_docs(
    Path((package, version)): Path<(OriginalName, Version)>,
    token: Token,
    State(state): AppState,
    mut docs: DocArchive,
) -> ApiResult<Json<DocUploadResponse>> {
    let db = state.db;
    let settings = state.settings;
    let normalized_name = package.to_normalized();
    let crate_version = &version.to_string();

    // Check if crate with the version exists.
    if let Some(id) = db.get_crate_id(&normalized_name).await? {
        if !db.crate_version_exists(id, crate_version).await? {
            return crate_does_not_exist(&normalized_name, crate_version);
        }
    } else {
        return crate_does_not_exist(&normalized_name, crate_version);
    }

    // Check if user from token is an owner of the crate.
    // If not, he is not allowed to push the docs.
    check_ownership(&normalized_name, &token, &db).await?;

    let doc_path = settings.docs_path().join(&*package).join(crate_version);

    let _ = tokio::task::spawn_blocking(move || docs.extract(&doc_path))
        .await
        .map_err(|_| ApiError::from("Failed to extract docs."))?;

    db.update_docs_link(
        &normalized_name,
        &version,
        &compute_doc_url(&package, &version),
    )
    .await?;

    Ok(Json(DocUploadResponse::new(
        "Successfully published docs.".to_string(),
        &package,
        &version,
    )))
}

fn crate_does_not_exist(
    crate_name: &str,
    crate_version: &str,
) -> ApiResult<Json<DocUploadResponse>> {
    Err(ApiError::from(&format!(
        "No Crate with version exists: {}-{}",
        crate_name, crate_version
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_queue_response::DocQueueEntryResponse;
    use appstate::AppStateData;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use common::normalized_name::NormalizedName;
    use db::mock::MockDb;
    use db::{DbProvider, DocQueueEntry};
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn doc_in_queue_returns_queue_entries() {
        let mut db = MockDb::new();
        db.expect_get_doc_queue().returning(|| {
            Ok(vec![
                DocQueueEntry {
                    id: 0,
                    krate: NormalizedName::from_unchecked("crate1".to_string()),
                    version: "0.0.1".to_string(),
                    path: Default::default(),
                },
                DocQueueEntry {
                    id: 1,
                    krate: NormalizedName::from_unchecked("crate2".to_string()),
                    version: "0.0.2".to_string(),
                    path: Default::default(),
                },
            ])
        });

        let kellnr = app(Arc::new(db)).await;
        let r = kellnr
            .oneshot(Request::get("/queue").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let actual = r.into_body().collect().await.unwrap().to_bytes();
        let actual = serde_json::from_slice::<DocQueueResponse>(&actual).unwrap();
        assert_eq!(
            DocQueueResponse {
                queue: vec![
                    DocQueueEntryResponse {
                        name: "crate1".to_string(),
                        version: "0.0.1".to_string()
                    },
                    DocQueueEntryResponse {
                        name: "crate2".to_string(),
                        version: "0.0.2".to_string()
                    }
                ]
            },
            actual
        );
    }

    async fn app(db: Arc<dyn DbProvider>) -> Router {
        Router::new()
            .route("/queue", get(docs_in_queue))
            .with_state(AppStateData {
                db,
                ..appstate::test_state().await
            })
    }
}
