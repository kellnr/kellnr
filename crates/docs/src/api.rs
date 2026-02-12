use axum::Json;
use axum::extract::{Path, State};
use axum::response::Redirect;
use kellnr_appstate::{AppState, DbState, SettingsState};
use kellnr_auth::token::Token;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_error::api_error::ApiResult;
use kellnr_registry::kellnr_api::check_ownership;

use crate::doc_archive::DocArchive;
use crate::doc_queue_response::DocQueueResponse;
use crate::docs_error::DocsError;
use crate::upload_response::DocUploadResponse;
use crate::{compute_doc_url, get_latest_version_with_doc};

/// Get documentation build queue
///
/// Returns the list of crates currently in the documentation build queue.
#[utoipa::path(
    get,
    path = "/builds",
    tag = "docs",
    responses(
        (status = 200, description = "Documentation build queue", body = DocQueueResponse)
    ),
    security(("session_cookie" = []))
)]
pub async fn docs_in_queue(State(db): DbState) -> ApiResult<Json<DocQueueResponse>> {
    let doc = db.get_doc_queue().await?;
    Ok(Json(DocQueueResponse::from(doc)))
}

/// Redirect to latest documentation
///
/// Redirects to the latest documentation for a given package.
#[utoipa::path(
    get,
    path = "/{package}/latest",
    tag = "docs",
    params(
        ("package" = String, Path, description = "Package name")
    ),
    responses(
        (status = 302, description = "Redirect to latest documentation")
    ),
    security(("session_cookie" = []))
)]
pub async fn latest_docs(
    Path(package): Path<OriginalName>,
    State(settings): SettingsState,
    State(db): DbState,
) -> Redirect {
    let name = package.to_normalized();
    let opt_doc_version = get_latest_version_with_doc(&name, &settings);
    let res_db_version = db.get_max_version_from_name(&name).await;

    if let Some(doc_version) = opt_doc_version
        && let Ok(db_version) = res_db_version
        && doc_version == db_version
    {
        return Redirect::temporary(&compute_doc_url(&name, &db_version, &settings.origin.path));
    }

    Redirect::temporary("/")
}

/// Publish documentation for a crate version
///
/// Upload documentation for a specific crate and version.
/// Requires ownership of the crate (via cargo token).
#[utoipa::path(
    put,
    path = "/{package}/{version}",
    tag = "docs",
    params(
        ("package" = String, Path, description = "Package name"),
        ("version" = String, Path, description = "Package version")
    ),
    request_body(content = Vec<u8>, description = "Documentation archive (tar.gz or zip)", content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Documentation published successfully", body = DocUploadResponse),
        (status = 400, description = "Crate or version does not exist"),
        (status = 401, description = "Not authorized"),
        (status = 403, description = "Not an owner of the crate")
    ),
    security(("cargo_token" = []))
)]
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
    let user = kellnr_auth::maybe_user::MaybeUser::from_token(token);
    check_ownership(&normalized_name, &user, &db).await?;

    let doc_path = settings.docs_path().join(&*package).join(crate_version);

    let _ = tokio::task::spawn_blocking(move || docs.extract(&doc_path))
        .await
        .map_err(|_| DocsError::ExtractFailed)?;

    db.update_docs_link(
        &normalized_name,
        &version,
        &compute_doc_url(&package, &version, &settings.origin.path),
    )
    .await?;

    Ok(Json(DocUploadResponse::new(
        "Successfully published docs.".to_string(),
        &package,
        &version,
        &settings.origin.path,
    )))
}

fn crate_does_not_exist(
    crate_name: &str,
    crate_version: &str,
) -> ApiResult<Json<DocUploadResponse>> {
    Err(DocsError::CrateDoesNotExist(crate_name.to_string(), crate_version.to_string()).into())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use http_body_util::BodyExt;
    use kellnr_appstate::AppStateData;
    use kellnr_common::normalized_name::NormalizedName;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{DbProvider, DocQueueEntry};
    use tower::ServiceExt;

    use super::*;
    use crate::doc_queue_response::DocQueueEntryResponse;

    #[tokio::test]
    async fn doc_in_queue_returns_queue_entries() {
        let mut db = MockDb::new();
        db.expect_get_doc_queue().returning(|| {
            Ok(vec![
                DocQueueEntry {
                    id: 0,
                    normalized_name: NormalizedName::from_unchecked("crate1".to_string()),
                    version: "0.0.1".to_string(),
                    path: PathBuf::default(),
                },
                DocQueueEntry {
                    id: 1,
                    normalized_name: NormalizedName::from_unchecked("crate2".to_string()),
                    version: "0.0.2".to_string(),
                    path: PathBuf::default(),
                },
            ])
        });

        let kellnr = app(Arc::new(db));
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

    fn app(db: Arc<dyn DbProvider>) -> Router {
        Router::new()
            .route("/queue", get(docs_in_queue))
            .with_state(AppStateData {
                db,
                ..kellnr_appstate::test_state()
            })
    }
}
