use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::response::Redirect;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::DbProvider;
use kellnr_http::authz::{can_read_crate, require_owner, require_writable_for_upload};
use kellnr_http::error::ApiResult;
use kellnr_http::identity::{AnyUser, Identity, TokenUser};
use kellnr_http::state::{AppState, DbState, DocsStorageState, SettingsState};
use kellnr_storage::docs_storage::DocsName;
use tracing::error;

use crate::doc_archive::DocArchive;
use crate::doc_queue_response::DocQueueResponse;
use crate::docs_error::DocsError;
use crate::upload::upload_dir_and_prune;
use crate::upload_response::DocUploadResponse;
use crate::{compute_doc_url, doc_exists};

/// Get documentation build queue
///
/// Returns the list of crates currently in the documentation build queue.
/// Download-restricted crates are only listed for users who may read their docs.
#[utoipa::path(
    get,
    path = "/builds",
    tag = "docs",
    responses(
        (status = 200, description = "Documentation build queue", body = DocQueueResponse)
    ),
    security(("session_cookie" = []))
)]
pub async fn docs_in_queue(
    user: Option<AnyUser>,
    State(db): DbState,
) -> ApiResult<Json<DocQueueResponse>> {
    let mut visible = Vec::new();
    for entry in db.get_doc_queue().await? {
        let Ok(name) = entry.canonical_name() else {
            continue;
        };
        if can_read_docs(&name, user.as_deref(), &db).await {
            visible.push(entry);
        }
    }
    Ok(Json(DocQueueResponse::from(visible)))
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
    user: Option<AnyUser>,
    State(settings): SettingsState,
    State(docs_storage): DocsStorageState,
    State(db): DbState,
) -> Redirect {
    let name = package.to_canonical();

    // Same response as "no docs" to not reveal that a restricted crate exists.
    if !can_read_docs(&name, user.as_deref(), &db).await {
        return Redirect::temporary("/");
    }

    if let Ok(version) = db.get_max_version_from_name(&name).await
        && let Ok(Some(original_name)) = db.get_original_name(&name, &version).await
        && doc_exists(&original_name, &version, &docs_storage).await
    {
        return Redirect::temporary(&compute_doc_url(
            &original_name,
            &version,
            &settings.origin.path_prefix(),
        ));
    }

    Redirect::temporary("/")
}

/// Whether `user` may read the docs of a crate, honoring its download restriction.
async fn can_read_docs(
    crate_name: &CanonicalName,
    user: Option<&Identity>,
    db: &Arc<dyn DbProvider>,
) -> bool {
    can_read_crate(crate_name, user, db)
        .await
        .unwrap_or_else(|e| {
            error!("Failed to check docs access for {crate_name}: {e}");
            false
        })
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
    user: TokenUser,
    State(state): AppState,
    mut docs: DocArchive,
) -> ApiResult<Json<DocUploadResponse>> {
    let db = state.db;
    let settings = state.settings;
    let docs_storage = state.docs_storage;
    let canonical_name = package.to_canonical();

    // Check if user from token is an owner of the crate.
    // If not, he is not allowed to push the docs. This comes before the
    // version check, so that others cannot tell which crates and versions
    // exist: they get the same error for a missing crate.
    require_writable_for_upload(&user)?;
    require_owner(&canonical_name, &user, &db).await?;

    let Some(original_name) = db.get_original_name(&canonical_name, &version).await? else {
        return crate_does_not_exist(&package, &version);
    };

    let tmp_dir = tempfile::tempdir().map_err(DocsError::IoError)?;
    let extract_path = tmp_dir.path().to_path_buf();

    tokio::task::spawn_blocking(move || docs.extract(&extract_path))
        .await
        .map_err(|_| DocsError::ExtractFailed)??;

    upload_dir_and_prune(
        tmp_dir.path(),
        "",
        &DocsName::from(&original_name),
        &version,
        &docs_storage,
    )
    .await?;

    db.update_docs_link(
        &canonical_name,
        &version,
        &compute_doc_url(&original_name, &version, &settings.origin.path_prefix()),
    )
    .await?;

    Ok(Json(DocUploadResponse::new(
        "Successfully published docs.".to_string(),
        &original_name,
        &version,
        &settings.origin.path_prefix(),
    )))
}

fn crate_does_not_exist(
    crate_name: &OriginalName,
    crate_version: &Version,
) -> ApiResult<Json<DocUploadResponse>> {
    Err(DocsError::CrateDoesNotExist(crate_name.to_lowercase(), crate_version.to_string()).into())
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
    use kellnr_common::account_name::UserName;
    use kellnr_common::canonical_name::CanonicalName;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{DbProvider, DocQueueEntry, User};
    use kellnr_http::state::AppStateData;
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::docs_storage::DocsStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::object_storage::MemoryStorage;
    use mockall::predicate::eq;
    use tower::ServiceExt;

    use super::*;
    use crate::doc_queue_response::DocQueueEntryResponse;

    #[tokio::test]
    async fn doc_in_queue_returns_queue_entries() {
        let mut db = MockDb::new();
        db.expect_is_download_restricted().returning(|_| Ok(false));
        db.expect_get_doc_queue().returning(|| {
            Ok(vec![
                DocQueueEntry {
                    id: 0,
                    name: "crate1".to_string(),
                    version: Version::try_from("0.0.1").unwrap(),
                    path: PathBuf::default(),
                },
                DocQueueEntry {
                    id: 1,
                    name: "crate2".to_string(),
                    version: Version::try_from("0.0.2").unwrap(),
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

    fn queue_entry(id: i64, name: &str) -> DocQueueEntry {
        DocQueueEntry {
            id,
            name: name.to_string(),
            version: Version::try_from("1.0.0").unwrap(),
            path: PathBuf::default(),
        }
    }

    /// Queue with the unrestricted crate `public` and the restricted crate `secret`.
    fn db_with_restricted_queue_entry() -> MockDb {
        let mut db = MockDb::new();
        db.expect_get_doc_queue()
            .returning(|| Ok(vec![queue_entry(0, "public"), queue_entry(1, "secret")]));
        db.expect_is_download_restricted()
            .returning(|name| Ok(name.to_string() == "secret"));
        db
    }

    async fn queued_names(db: MockDb, token: Option<&str>) -> Vec<String> {
        let mut request = Request::get("/queue");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, token);
        }
        let r = app(Arc::new(db))
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = r.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice::<DocQueueResponse>(&body)
            .unwrap()
            .queue
            .into_iter()
            .map(|e| e.name)
            .collect()
    }

    #[tokio::test]
    async fn doc_queue_hides_restricted_crate_from_anonymous_user() {
        let names = queued_names(db_with_restricted_queue_entry(), None).await;

        assert_eq!(names, vec!["public"]);
    }

    #[tokio::test]
    async fn doc_queue_hides_restricted_crate_from_user_without_access() {
        let mut db = db_with_restricted_queue_entry();
        expect_token_user(&mut db, "bob");
        db.expect_is_crate_user().returning(|_, _| Ok(false));
        db.expect_is_crate_group_user().returning(|_, _| Ok(false));
        db.expect_is_owner().returning(|_, _| Ok(false));

        let names = queued_names(db, Some("token")).await;

        assert_eq!(names, vec!["public"]);
    }

    #[tokio::test]
    async fn doc_queue_lists_restricted_crate_for_crate_user() {
        let mut db = db_with_restricted_queue_entry();
        expect_token_user(&mut db, "alice");
        db.expect_is_crate_user()
            .returning(|_, user| Ok(user == "alice"));

        let names = queued_names(db, Some("token")).await;

        assert_eq!(names, vec!["public", "secret"]);
    }

    fn app(db: Arc<dyn DbProvider>) -> Router {
        Router::new()
            .route("/queue", get(docs_in_queue))
            .with_state(AppStateData {
                db,
                ..kellnr_http::test_support::test_state()
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
                ..kellnr_http::test_support::test_state()
            })
    }

    fn expect_admin_owner(db: &mut MockDb, canonical: &CanonicalName) {
        expect_original_name(db, canonical, "test-crate");
        db.expect_get_user_from_token().returning(|_| {
            Ok(User {
                is_admin: true,
                name: UserName::new("admin").unwrap(),
                ..User::default()
            })
        });
    }

    fn expect_original_name(db: &mut MockDb, canonical: &CanonicalName, original: &'static str) {
        db.expect_get_original_name()
            .with(
                eq(canonical.clone()),
                eq(Version::try_from("1.0.0").unwrap()),
            )
            .returning(move |_, _| Ok(Some(OriginalName::from_unchecked(original.to_string()))));
    }

    #[tokio::test]
    async fn publish_docs_stores_docs_under_lowercase_name_and_links_rustdoc_folder() {
        let canonical = CanonicalName::try_from("my-crate").unwrap();
        let mut db = MockDb::new();
        expect_original_name(&mut db, &canonical, "My-Crate");
        db.expect_get_user_from_token().returning(|_| {
            Ok(User {
                is_admin: true,
                name: UserName::new("admin").unwrap(),
                ..User::default()
            })
        });
        db.expect_update_docs_link()
            .with(
                eq(canonical.clone()),
                eq(Version::try_from("1.0.0").unwrap()),
                eq("/docs/my-crate/1.0.0/doc/My_Crate/index.html".to_string()),
            )
            .returning(|_, _, _| Ok(()));
        let docs_storage = Arc::new(DocsStorage::new(
            Box::new(MemoryStorage::new()) as DynStorage
        ));

        let r = publish_app(Arc::new(db), docs_storage.clone())
            .oneshot(
                Request::put("/MY-crate/1.0.0")
                    .header(header::AUTHORIZATION, "sometoken")
                    .body(Body::from(build_zip(&[(
                        "doc/My_Crate/index.html",
                        b"<html>docs</html>",
                    )])))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
        let key = DocsStorage::file_key(
            &docs_name("my-crate"),
            &Version::try_from("1.0.0").unwrap(),
            "doc/My_Crate/index.html",
        );
        assert!(docs_storage.exists(&key).await.unwrap());
        let body = r.into_body().collect().await.unwrap().to_bytes();
        let response: DocUploadResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(response.crate_name, "My-Crate");
    }

    // Docs stay under the lowercase name the crate was published with, also
    // if they are uploaded under another spelling of the crate name.
    #[tokio::test]
    async fn publish_docs_under_other_separator_stores_docs_under_published_name() {
        let canonical = CanonicalName::try_from("my-crate").unwrap();
        let mut db = MockDb::new();
        expect_original_name(&mut db, &canonical, "My-Crate");
        db.expect_get_user_from_token().returning(|_| {
            Ok(User {
                is_admin: true,
                name: UserName::new("admin").unwrap(),
                ..User::default()
            })
        });
        db.expect_update_docs_link()
            .with(
                eq(canonical.clone()),
                eq(Version::try_from("1.0.0").unwrap()),
                eq("/docs/my-crate/1.0.0/doc/My_Crate/index.html".to_string()),
            )
            .returning(|_, _, _| Ok(()));
        let docs_storage = Arc::new(DocsStorage::new(
            Box::new(MemoryStorage::new()) as DynStorage
        ));

        let r = publish_app(Arc::new(db), docs_storage.clone())
            .oneshot(
                Request::put("/my_crate/1.0.0")
                    .header(header::AUTHORIZATION, "sometoken")
                    .body(Body::from(build_zip(&[(
                        "doc/My_Crate/index.html",
                        b"<html>docs</html>",
                    )])))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
        let version = Version::try_from("1.0.0").unwrap();
        let key =
            DocsStorage::file_key(&docs_name("my-crate"), &version, "doc/My_Crate/index.html");
        assert!(docs_storage.exists(&key).await.unwrap());
        let other = DocsStorage::version_prefix(&docs_name("my_crate"), &version);
        assert!(docs_storage.list(&other).await.unwrap().is_empty());
    }

    fn docs_name(name: &str) -> DocsName {
        DocsName::from(&OriginalName::try_from(name).unwrap())
    }

    #[tokio::test]
    async fn publish_docs_uploads_files_and_updates_docs_link() {
        let canonical = CanonicalName::try_from("test-crate").unwrap();
        let mut db = MockDb::new();
        expect_admin_owner(&mut db, &canonical);
        db.expect_update_docs_link()
            .with(
                eq(canonical.clone()),
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

        let key = DocsStorage::file_key(
            &docs_name("test-crate"),
            &Version::try_from("1.0.0").unwrap(),
            "doc/test_crate/index.html",
        );
        let stored = docs_storage.get(&key).await.unwrap();
        assert_eq!(stored, bytes::Bytes::from_static(b"<html>docs</html>"));
    }

    #[tokio::test]
    async fn publish_docs_rejects_read_only_owner() {
        let canonical = CanonicalName::try_from("test-crate").unwrap();
        let mut db = MockDb::new();
        expect_original_name(&mut db, &canonical, "test-crate");
        db.expect_get_user_from_token().returning(|_| {
            Ok(User {
                name: UserName::new("reader").unwrap(),
                is_read_only: true,
                ..User::default()
            })
        });
        db.expect_is_owner().returning(|_, _| Ok(true));
        db.expect_update_docs_link().never();

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

        // Read-only users are rejected with 400, like on all other modifying endpoints.
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        let key = DocsStorage::file_key(
            &docs_name("test-crate"),
            &Version::try_from("1.0.0").unwrap(),
            "doc/test_crate/index.html",
        );
        assert!(!docs_storage.exists(&key).await.unwrap());
    }

    // A non-owner gets the same response for a missing crate or version as for
    // an existing crate, so that the existence of a crate is not revealed.
    #[tokio::test]
    async fn publish_docs_non_owner_cannot_tell_missing_from_existing_crate() {
        let mut db = MockDb::new();
        db.expect_get_original_name().returning(|name, version| {
            Ok(
                (name.as_str() == "test-crate" && version.to_string() == "1.0.0")
                    .then(|| OriginalName::from_unchecked(name.to_string())),
            )
        });
        db.expect_get_user_from_token().returning(|_| {
            Ok(User {
                name: UserName::new("other").unwrap(),
                ..User::default()
            })
        });
        db.expect_is_owner().returning(|_, _| Ok(false));
        db.expect_update_docs_link().never();
        let db: Arc<dyn DbProvider> = Arc::new(db);

        let tmp = tempfile::tempdir().unwrap();
        let docs_storage = Arc::new(DocsStorage::new(Box::new(
            FSStorage::new(tmp.path().to_str().unwrap()).unwrap(),
        ) as DynStorage));

        for uri in ["/test-crate/1.0.0", "/test-crate/9.9.9", "/missing/1.0.0"] {
            let zip_bytes = build_zip(&[("doc/x/index.html", b"<html>docs</html>")]);
            let r = publish_app(db.clone(), docs_storage.clone())
                .oneshot(
                    Request::put(uri)
                        .header(header::AUTHORIZATION, "sometoken")
                        .body(Body::from(zip_bytes))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::FORBIDDEN, "{uri}");
        }
    }

    #[tokio::test]
    async fn publish_docs_propagates_extraction_failure() {
        // A well-formed zip whose only entry escapes the extraction directory. The
        // archive parses fine (`ZipArchive::new` succeeds) but `ZipArchive::extract`
        // rejects the unsafe path, so the inner `Result` from `docs.extract(..)` must
        // reach the caller as an error (previously silently discarded, returning 200).
        let canonical = CanonicalName::try_from("test-crate").unwrap();
        let mut db = MockDb::new();
        expect_admin_owner(&mut db, &canonical);

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

    fn latest_app(db: Arc<dyn DbProvider>, docs_storage: Arc<DocsStorage>) -> Router {
        Router::new()
            .route("/{package}/latest", get(latest_docs))
            .with_state(AppStateData {
                db,
                docs_storage,
                ..kellnr_http::test_support::test_state()
            })
    }

    async fn docs_storage_with_latest_docs() -> (tempfile::TempDir, Arc<DocsStorage>) {
        let tmp = tempfile::tempdir().unwrap();
        let docs_storage = Arc::new(DocsStorage::new(Box::new(
            FSStorage::new(tmp.path().to_str().unwrap()).unwrap(),
        ) as DynStorage));
        let key = DocsStorage::file_key(
            &docs_name("secret"),
            &Version::try_from("1.0.0").unwrap(),
            "doc/secret/index.html",
        );
        docs_storage
            .put(&key, bytes::Bytes::from_static(b"<html>docs</html>"))
            .await
            .unwrap();
        (tmp, docs_storage)
    }

    fn expect_restricted_crate(db: &mut MockDb, restricted: bool) {
        db.expect_is_download_restricted()
            .returning(move |_| Ok(restricted));
        db.expect_get_max_version_from_name()
            .returning(|_| Ok(Version::try_from("1.0.0").unwrap()));
        db.expect_get_original_name()
            .returning(|_, _| Ok(Some(OriginalName::from_unchecked("secret".to_string()))));
    }

    fn expect_token_user(db: &mut MockDb, name: &'static str) {
        db.expect_get_user_from_token().returning(move |_| {
            Ok(User {
                name: UserName::new(name).unwrap(),
                ..User::default()
            })
        });
    }

    async fn latest_location(
        db: MockDb,
        docs_storage: Arc<DocsStorage>,
        token: Option<&str>,
    ) -> String {
        let mut request = Request::get("/secret/latest");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, token);
        }
        let r = latest_app(Arc::new(db), docs_storage)
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::TEMPORARY_REDIRECT);
        r.headers()[header::LOCATION].to_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn latest_docs_redirects_to_docs_for_unrestricted_crate() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let mut db = MockDb::new();
        expect_restricted_crate(&mut db, false);

        let location = latest_location(db, docs_storage, None).await;

        assert_eq!(location, "/docs/secret/1.0.0/doc/secret/index.html");
    }

    #[tokio::test]
    async fn latest_docs_redirects_to_docs_of_crate_with_uppercase_letters() {
        let docs_storage = Arc::new(DocsStorage::new(
            Box::new(MemoryStorage::new()) as DynStorage
        ));
        let key = DocsStorage::file_key(
            &docs_name("my-crate"),
            &Version::try_from("1.0.0").unwrap(),
            "doc/My_Crate/index.html",
        );
        docs_storage
            .put(&key, bytes::Bytes::from_static(b"<html>docs</html>"))
            .await
            .unwrap();
        let mut db = MockDb::new();
        db.expect_is_download_restricted().returning(|_| Ok(false));
        db.expect_get_max_version_from_name()
            .returning(|_| Ok(Version::try_from("1.0.0").unwrap()));
        expect_original_name(
            &mut db,
            &CanonicalName::try_from("my-crate").unwrap(),
            "My-Crate",
        );

        let r = latest_app(Arc::new(db), docs_storage)
            .oneshot(
                Request::get("/My-Crate/latest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            r.headers()[header::LOCATION],
            "/docs/my-crate/1.0.0/doc/My_Crate/index.html"
        );
    }

    #[tokio::test]
    async fn latest_docs_hides_restricted_crate_from_anonymous_user() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let mut db = MockDb::new();
        expect_restricted_crate(&mut db, true);

        let location = latest_location(db, docs_storage, None).await;

        assert_eq!(location, "/");
    }

    #[tokio::test]
    async fn latest_docs_hides_restricted_crate_from_user_without_access() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let mut db = MockDb::new();
        expect_restricted_crate(&mut db, true);
        expect_token_user(&mut db, "bob");
        db.expect_is_crate_user().returning(|_, _| Ok(false));
        db.expect_is_crate_group_user().returning(|_, _| Ok(false));
        db.expect_is_owner().returning(|_, _| Ok(false));

        let location = latest_location(db, docs_storage, Some("token")).await;

        assert_eq!(location, "/");
    }

    #[tokio::test]
    async fn latest_docs_redirects_crate_user_of_restricted_crate() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let mut db = MockDb::new();
        expect_restricted_crate(&mut db, true);
        expect_token_user(&mut db, "alice");
        db.expect_is_crate_user()
            .with(
                eq(CanonicalName::try_from("secret").unwrap()),
                eq(UserName::new("alice").unwrap()),
            )
            .returning(|_, _| Ok(true));

        let location = latest_location(db, docs_storage, Some("token")).await;

        assert_eq!(location, "/docs/secret/1.0.0/doc/secret/index.html");
    }

    #[tokio::test]
    async fn latest_docs_redirects_owner_of_restricted_crate() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let mut db = MockDb::new();
        expect_restricted_crate(&mut db, true);
        expect_token_user(&mut db, "olivia");
        db.expect_is_crate_user().returning(|_, _| Ok(false));
        db.expect_is_crate_group_user().returning(|_, _| Ok(false));
        db.expect_is_owner()
            .with(
                eq(CanonicalName::try_from("secret").unwrap()),
                eq(UserName::new("olivia").unwrap()),
            )
            .returning(|_, _| Ok(true));

        let location = latest_location(db, docs_storage, Some("token")).await;

        assert_eq!(location, "/docs/secret/1.0.0/doc/secret/index.html");
    }

    #[tokio::test]
    async fn latest_docs_redirects_group_member_of_restricted_crate() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let mut db = MockDb::new();
        expect_restricted_crate(&mut db, true);
        expect_token_user(&mut db, "greg");
        db.expect_is_crate_user().returning(|_, _| Ok(false));
        db.expect_is_crate_group_user()
            .with(
                eq(CanonicalName::try_from("secret").unwrap()),
                eq(UserName::new("greg").unwrap()),
            )
            .returning(|_, _| Ok(true));

        let location = latest_location(db, docs_storage, Some("token")).await;

        assert_eq!(location, "/docs/secret/1.0.0/doc/secret/index.html");
    }

    #[tokio::test]
    async fn latest_docs_hides_crate_when_restriction_check_fails() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let mut db = MockDb::new();
        db.expect_is_download_restricted()
            .returning(|_| Err(DbError::CrateNotFound("secret".to_string())));

        let location = latest_location(db, docs_storage, None).await;

        assert_eq!(location, "/");
    }

    #[tokio::test]
    async fn latest_docs_hides_restricted_crate_when_access_check_fails() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let mut db = MockDb::new();
        expect_restricted_crate(&mut db, true);
        expect_token_user(&mut db, "alice");
        db.expect_is_crate_user()
            .returning(|_, _| Err(DbError::CrateNotFound("secret".to_string())));

        let location = latest_location(db, docs_storage, Some("token")).await;

        assert_eq!(location, "/");
    }
}
