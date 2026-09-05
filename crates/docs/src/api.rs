use axum::Json;
use axum::extract::{Path, State};
use axum::response::Redirect;
use kellnr_appstate::{AppState, DbState, DocsStorageState, SettingsState};
use kellnr_auth::token::Token;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_error::api_error::ApiResult;
use kellnr_registry::kellnr_api::check_ownership;

use crate::doc_archive::DocArchive;
use crate::doc_queue_response::DocQueueResponse;
use crate::docs_error::DocsError;
use crate::upload::upload_dir_and_prune;
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
    State(docs_storage): DocsStorageState,
    State(db): DbState,
) -> Redirect {
    let name = package.to_normalized();
    let opt_doc_version = get_latest_version_with_doc(&name, &docs_storage).await;
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
    let docs_storage = state.docs_storage;
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

    let tmp_dir = tempfile::tempdir().map_err(DocsError::IoError)?;
    let extract_path = tmp_dir.path().to_path_buf();

    tokio::task::spawn_blocking(move || docs.extract(&extract_path))
        .await
        .map_err(|_| DocsError::ExtractFailed)??;

    upload_dir_and_prune(tmp_dir.path(), "", &package, crate_version, &docs_storage).await?;

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
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::routing::{get, put};
    use http_body_util::BodyExt;
    use kellnr_appstate::AppStateData;
    use kellnr_common::normalized_name::NormalizedName;
    use kellnr_db::User;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{DbProvider, DocQueueEntry};
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::docs_storage::DocsStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use mockall::predicate::eq;
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

    fn build_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default();
        for (name, content) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
        buf
    }

    fn publish_app(db: Arc<dyn DbProvider>, docs_storage: Arc<DocsStorage>) -> Router {
        Router::new()
            .route("/{package}/{version}", put(publish_docs))
            .with_state(AppStateData {
                db,
                docs_storage,
                ..kellnr_appstate::test_state()
            })
    }

    fn expect_admin_owner(db: &mut MockDb, normalized: &NormalizedName) {
        db.expect_get_crate_id()
            .with(eq(normalized.clone()))
            .returning(|_| Ok(Some(1)));
        db.expect_crate_version_exists()
            .with(eq(1), eq("1.0.0"))
            .returning(|_, _| Ok(true));
        db.expect_get_user_from_token().returning(|_| {
            Ok(User {
                is_admin: true,
                name: "admin".to_string(),
                ..User::default()
            })
        });
    }

    #[tokio::test]
    async fn publish_docs_uploads_files_and_updates_docs_link() {
        let normalized = NormalizedName::from_unchecked("test-crate".to_string());
        let mut db = MockDb::new();
        expect_admin_owner(&mut db, &normalized);
        db.expect_update_docs_link()
            .with(
                eq(normalized.clone()),
                eq(Version::try_from("1.0.0").unwrap()),
                eq("/docs/test-crate/1.0.0/doc/test_crate/index.html".to_string()),
            )
            .returning(|_, _, _| Ok(()));

        let tmp = tempfile::tempdir().unwrap();
        let docs_storage = Arc::new(DocsStorage::new(Box::new(
            FSStorage::new(tmp.path().to_str().unwrap()).unwrap(),
        ) as DynStorage));

        let kellnr = publish_app(Arc::new(db), docs_storage.clone());
        let zip_bytes = build_zip(&[("doc/test_crate/index.html", b"<html>docs</html>")]);

        let r = kellnr
            .oneshot(
                Request::put("/test-crate/1.0.0")
                    .header(header::AUTHORIZATION, "sometoken")
                    .body(Body::from(zip_bytes))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);

        let key = DocsStorage::file_key("test-crate", "1.0.0", "doc/test_crate/index.html");
        let stored = docs_storage.get(&key).await.unwrap();
        assert_eq!(stored, bytes::Bytes::from_static(b"<html>docs</html>"));
    }

    #[tokio::test]
    async fn publish_docs_propagates_extraction_failure() {
        // A well-formed zip whose only entry escapes the extraction directory. The
        // archive parses fine (`ZipArchive::new` succeeds) but `ZipArchive::extract`
        // rejects the unsafe path, so the inner `Result` from `docs.extract(..)` must
        // reach the caller as an error (previously silently discarded, returning 200).
        let normalized = NormalizedName::from_unchecked("test-crate".to_string());
        let mut db = MockDb::new();
        expect_admin_owner(&mut db, &normalized);

        let tmp = tempfile::tempdir().unwrap();
        let docs_storage = Arc::new(DocsStorage::new(Box::new(
            FSStorage::new(tmp.path().to_str().unwrap()).unwrap(),
        ) as DynStorage));

        let kellnr = publish_app(Arc::new(db), docs_storage);
        let zip_bytes = build_zip(&[("../escape.html", b"evil")]);

        let r = kellnr
            .oneshot(
                Request::put("/test-crate/1.0.0")
                    .header(header::AUTHORIZATION, "sometoken")
                    .body(Body::from(zip_bytes))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Previously this error was silently discarded (`let _ = ...`), returning 200
        // even though extraction failed. It must now surface as a client error.
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }
}
