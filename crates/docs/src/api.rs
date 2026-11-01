use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::response::Redirect;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::DbProvider;
use kellnr_http::authz::{can_read_crate, require_owner, require_writable};
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
    require_writable(&user)?;
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
    use kellnr_common::account_name::{GroupName, UserName};
    use kellnr_common::canonical_name::CanonicalName;
    use kellnr_common::plain_token::PlainToken;
    use kellnr_db::UserFlags;
    use kellnr_db::test_utils::{TestCrateBuilder, test_drop_table};
    use kellnr_http::state::AppStateData;
    use kellnr_http::test_support::sqlite_test_state;
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::docs_storage::DocsStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::object_storage::MemoryStorage;
    use kellnr_test_db::TestDb;
    use tower::ServiceExt;

    use super::*;
    use crate::doc_queue_response::DocQueueEntryResponse;

    /// Token of the user `admin` in every test database.
    const ADMIN_TOKEN: &str = "token";

    /// The docs API routes at their paths in the API, on `state` with `docs_storage`.
    fn app(state: AppStateData, docs_storage: Arc<DocsStorage>) -> Router {
        Router::new()
            .route("/builds", get(docs_in_queue))
            .route("/{package}/latest", get(latest_docs))
            .route("/{package}/{version}", put(publish_docs))
            .with_state(AppStateData {
                docs_storage,
                ..state
            })
    }

    fn memory_docs_storage() -> Arc<DocsStorage> {
        Arc::new(DocsStorage::new(
            Box::new(MemoryStorage::new()) as DynStorage
        ))
    }

    fn canonical(name: &str) -> CanonicalName {
        CanonicalName::try_from(name).unwrap()
    }

    fn user_name(name: &str) -> UserName {
        UserName::new(name).unwrap()
    }

    async fn add_crate(db: &TestDb, name: &str, version: &str) {
        TestCrateBuilder::new(db)
            .name(name)
            .version(version)
            .build()
            .await
            .unwrap();
    }

    /// Adds the crate `name`, which is download-restricted if `restricted`.
    async fn add_restricted_crate(db: &TestDb, name: &str, restricted: bool) {
        add_crate(db, name, "1.0.0").await;
        db.change_download_restricted(&canonical(name), restricted)
            .await
            .unwrap();
    }

    /// Adds the user `name` with `flags` and returns a new cargo token of it.
    async fn token_user(db: &TestDb, name: &str, flags: UserFlags) -> String {
        db.add_user(&user_name(name), "pwd", flags).await.unwrap();
        let token = format!("{name}-token");
        db.add_auth_token("test", &PlainToken::from(token.as_str()), &user_name(name))
            .await
            .unwrap();
        token
    }

    async fn add_doc_queue(db: &TestDb, name: &str, version: &str) {
        db.add_doc_queue(
            &OriginalName::try_from(name).unwrap(),
            &Version::try_from(version).unwrap(),
            &PathBuf::default(),
        )
        .await
        .unwrap();
    }

    async fn docs_link(db: &TestDb, name: &str) -> Option<String> {
        let data = db.get_crate_data(&canonical(name)).await.unwrap();
        data.versions[0].documentation.clone()
    }

    #[tokio::test]
    async fn doc_in_queue_returns_queue_entries() {
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "crate1", "0.0.1").await;
        add_crate(&db, "crate2", "0.0.2").await;
        add_doc_queue(&db, "crate1", "0.0.1").await;
        add_doc_queue(&db, "crate2", "0.0.2").await;

        let r = app(state, memory_docs_storage())
            .oneshot(Request::get("/builds").body(Body::empty()).unwrap())
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

    /// App state with the unrestricted crate `public` and the restricted
    /// crate `secret` in the docs queue.
    async fn state_with_restricted_queue_entry() -> (AppStateData, TestDb) {
        let (state, db) = sqlite_test_state().await;
        add_restricted_crate(&db, "public", false).await;
        add_restricted_crate(&db, "secret", true).await;
        add_doc_queue(&db, "public", "1.0.0").await;
        add_doc_queue(&db, "secret", "1.0.0").await;
        (state, db)
    }

    async fn queued_names(state: AppStateData, token: Option<&str>) -> Vec<String> {
        let mut request = Request::get("/builds");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, token);
        }
        let r = app(state, memory_docs_storage())
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
        let (state, _db) = state_with_restricted_queue_entry().await;

        let names = queued_names(state, None).await;

        assert_eq!(names, vec!["public"]);
    }

    #[tokio::test]
    async fn doc_queue_hides_restricted_crate_from_user_without_access() {
        let (state, db) = state_with_restricted_queue_entry().await;
        let token = token_user(&db, "bob", UserFlags::normal()).await;

        let names = queued_names(state, Some(&token)).await;

        assert_eq!(names, vec!["public"]);
    }

    #[tokio::test]
    async fn doc_queue_lists_restricted_crate_for_crate_user() {
        let (state, db) = state_with_restricted_queue_entry().await;
        let token = token_user(&db, "alice", UserFlags::normal()).await;
        db.add_crate_user(&canonical("secret"), &user_name("alice"))
            .await
            .unwrap();

        let names = queued_names(state, Some(&token)).await;

        assert_eq!(names, vec!["public", "secret"]);
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

    fn fs_docs_storage(dir: &tempfile::TempDir) -> Arc<DocsStorage> {
        Arc::new(DocsStorage::new(
            Box::new(FSStorage::new(dir.path().to_str().unwrap()).unwrap()) as DynStorage,
        ))
    }

    fn publish_request(uri: &str, token: &str, zip: Vec<u8>) -> Request<Body> {
        Request::put(uri)
            .header(header::AUTHORIZATION, token)
            .body(Body::from(zip))
            .unwrap()
    }

    #[tokio::test]
    async fn publish_docs_stores_docs_under_lowercase_name_and_links_rustdoc_folder() {
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "My-Crate", "1.0.0").await;
        let docs_storage = memory_docs_storage();
        let zip = build_zip(&[("doc/My_Crate/index.html", b"<html>docs</html>")]);

        let r = app(state, docs_storage.clone())
            .oneshot(publish_request("/MY-crate/1.0.0", ADMIN_TOKEN, zip))
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
        assert_eq!(
            docs_link(&db, "my-crate").await.as_deref(),
            Some("/docs/my-crate/1.0.0/doc/My_Crate/index.html")
        );
    }

    // Docs stay under the lowercase name the crate was published with, also
    // if they are uploaded under another spelling of the crate name.
    #[tokio::test]
    async fn publish_docs_under_other_separator_stores_docs_under_published_name() {
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "My-Crate", "1.0.0").await;
        let docs_storage = memory_docs_storage();
        let zip = build_zip(&[("doc/My_Crate/index.html", b"<html>docs</html>")]);

        let r = app(state, docs_storage.clone())
            .oneshot(publish_request("/my_crate/1.0.0", ADMIN_TOKEN, zip))
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
        let version = Version::try_from("1.0.0").unwrap();
        let key =
            DocsStorage::file_key(&docs_name("my-crate"), &version, "doc/My_Crate/index.html");
        assert!(docs_storage.exists(&key).await.unwrap());
        let other = DocsStorage::version_prefix(&docs_name("my_crate"), &version);
        assert!(docs_storage.list(&other).await.unwrap().is_empty());
        assert_eq!(
            docs_link(&db, "my-crate").await.as_deref(),
            Some("/docs/my-crate/1.0.0/doc/My_Crate/index.html")
        );
    }

    fn docs_name(name: &str) -> DocsName {
        DocsName::from(&OriginalName::try_from(name).unwrap())
    }

    #[tokio::test]
    async fn publish_docs_uploads_files_and_updates_docs_link() {
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "test-crate", "1.0.0").await;
        let tmp = tempfile::tempdir().unwrap();
        let docs_storage = fs_docs_storage(&tmp);
        let zip = build_zip(&[("doc/test_crate/index.html", b"<html>docs</html>")]);

        let r = app(state, docs_storage.clone())
            .oneshot(publish_request("/test-crate/1.0.0", ADMIN_TOKEN, zip))
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
        assert_eq!(
            docs_link(&db, "test-crate").await.as_deref(),
            Some("/docs/test-crate/1.0.0/doc/test_crate/index.html")
        );
    }

    #[tokio::test]
    async fn publish_docs_rejects_read_only_owner() {
        let (state, db) = sqlite_test_state().await;
        let token = token_user(&db, "reader", UserFlags::read_only()).await;
        TestCrateBuilder::new(&db)
            .name("test-crate")
            .owner("reader")
            .build()
            .await
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let docs_storage = fs_docs_storage(&tmp);
        let zip = build_zip(&[("doc/test_crate/index.html", b"<html>docs</html>")]);

        let r = app(state, docs_storage.clone())
            .oneshot(publish_request("/test-crate/1.0.0", &token, zip))
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        let key = DocsStorage::file_key(
            &docs_name("test-crate"),
            &Version::try_from("1.0.0").unwrap(),
            "doc/test_crate/index.html",
        );
        assert!(!docs_storage.exists(&key).await.unwrap());
        assert_eq!(docs_link(&db, "test-crate").await, None);
    }

    // A non-owner gets the same response for a missing crate or version as for
    // an existing crate, so that the existence of a crate is not revealed.
    #[tokio::test]
    async fn publish_docs_non_owner_cannot_tell_missing_from_existing_crate() {
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "test-crate", "1.0.0").await;
        let token = token_user(&db, "other", UserFlags::normal()).await;
        let tmp = tempfile::tempdir().unwrap();
        let docs_storage = fs_docs_storage(&tmp);

        for uri in ["/test-crate/1.0.0", "/test-crate/9.9.9", "/missing/1.0.0"] {
            let zip = build_zip(&[("doc/x/index.html", b"<html>docs</html>")]);
            let r = app(state.clone(), docs_storage.clone())
                .oneshot(publish_request(uri, &token, zip))
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::FORBIDDEN, "{uri}");
        }
        assert_eq!(docs_link(&db, "test-crate").await, None);
    }

    #[tokio::test]
    async fn publish_docs_propagates_extraction_failure() {
        // A well-formed zip whose only entry escapes the extraction directory. The
        // archive parses fine (`ZipArchive::new` succeeds) but `ZipArchive::extract`
        // rejects the unsafe path, so the inner `Result` from `docs.extract(..)` must
        // reach the caller as an error (previously silently discarded, returning 200).
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "test-crate", "1.0.0").await;
        let tmp = tempfile::tempdir().unwrap();
        let zip = build_zip(&[("../escape.html", b"evil")]);

        let r = app(state, fs_docs_storage(&tmp))
            .oneshot(publish_request("/test-crate/1.0.0", ADMIN_TOKEN, zip))
            .await
            .unwrap();

        // Previously this error was silently discarded (`let _ = ...`), returning 200
        // even though extraction failed. It must now surface as a client error.
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        assert_eq!(docs_link(&db, "test-crate").await, None);
    }

    async fn docs_storage_with_latest_docs() -> (tempfile::TempDir, Arc<DocsStorage>) {
        let tmp = tempfile::tempdir().unwrap();
        let docs_storage = fs_docs_storage(&tmp);
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

    /// App state with the crate `secret`, which is download-restricted if `restricted`.
    async fn state_with_crate(restricted: bool) -> (AppStateData, TestDb) {
        let (state, db) = sqlite_test_state().await;
        add_restricted_crate(&db, "secret", restricted).await;
        (state, db)
    }

    async fn latest_location(
        state: AppStateData,
        docs_storage: Arc<DocsStorage>,
        token: Option<&str>,
    ) -> String {
        let mut request = Request::get("/secret/latest");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, token);
        }
        let r = app(state, docs_storage)
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::TEMPORARY_REDIRECT);
        r.headers()[header::LOCATION].to_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn latest_docs_redirects_to_docs_for_unrestricted_crate() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let (state, _db) = state_with_crate(false).await;

        let location = latest_location(state, docs_storage, None).await;

        assert_eq!(location, "/docs/secret/1.0.0/doc/secret/index.html");
    }

    #[tokio::test]
    async fn latest_docs_redirects_to_docs_of_crate_with_uppercase_letters() {
        let docs_storage = memory_docs_storage();
        let key = DocsStorage::file_key(
            &docs_name("my-crate"),
            &Version::try_from("1.0.0").unwrap(),
            "doc/My_Crate/index.html",
        );
        docs_storage
            .put(&key, bytes::Bytes::from_static(b"<html>docs</html>"))
            .await
            .unwrap();
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "My-Crate", "1.0.0").await;

        let r = app(state, docs_storage)
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
        let (state, _db) = state_with_crate(true).await;

        let location = latest_location(state, docs_storage, None).await;

        assert_eq!(location, "/");
    }

    #[tokio::test]
    async fn latest_docs_hides_restricted_crate_from_user_without_access() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let (state, db) = state_with_crate(true).await;
        let token = token_user(&db, "bob", UserFlags::normal()).await;

        let location = latest_location(state, docs_storage, Some(&token)).await;

        assert_eq!(location, "/");
    }

    #[tokio::test]
    async fn latest_docs_redirects_crate_user_of_restricted_crate() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let (state, db) = state_with_crate(true).await;
        let token = token_user(&db, "alice", UserFlags::normal()).await;
        db.add_crate_user(&canonical("secret"), &user_name("alice"))
            .await
            .unwrap();

        let location = latest_location(state, docs_storage, Some(&token)).await;

        assert_eq!(location, "/docs/secret/1.0.0/doc/secret/index.html");
    }

    #[tokio::test]
    async fn latest_docs_redirects_owner_of_restricted_crate() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let (state, db) = state_with_crate(true).await;
        let token = token_user(&db, "olivia", UserFlags::normal()).await;
        db.add_owner(&canonical("secret"), &user_name("olivia"))
            .await
            .unwrap();

        let location = latest_location(state, docs_storage, Some(&token)).await;

        assert_eq!(location, "/docs/secret/1.0.0/doc/secret/index.html");
    }

    #[tokio::test]
    async fn latest_docs_redirects_group_member_of_restricted_crate() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let (state, db) = state_with_crate(true).await;
        let token = token_user(&db, "greg", UserFlags::normal()).await;
        let group = GroupName::new("readers").unwrap();
        db.add_group(&group).await.unwrap();
        db.add_group_user(&group, &user_name("greg")).await.unwrap();
        db.add_crate_group(&canonical("secret"), &group)
            .await
            .unwrap();

        let location = latest_location(state, docs_storage, Some(&token)).await;

        assert_eq!(location, "/docs/secret/1.0.0/doc/secret/index.html");
    }

    #[tokio::test]
    async fn latest_docs_hides_crate_when_restriction_check_fails() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let (state, db) = state_with_crate(false).await;
        test_drop_table(&db, "krate").await.unwrap();

        let location = latest_location(state, docs_storage, None).await;

        assert_eq!(location, "/");
    }

    #[tokio::test]
    async fn latest_docs_hides_restricted_crate_when_access_check_fails() {
        let (_tmp, docs_storage) = docs_storage_with_latest_docs().await;
        let (state, db) = state_with_crate(true).await;
        // Owners may read the docs, but the crate users are checked first.
        let token = token_user(&db, "olivia", UserFlags::normal()).await;
        db.add_owner(&canonical("secret"), &user_name("olivia"))
            .await
            .unwrap();
        test_drop_table(&db, "crate_user").await.unwrap();

        let location = latest_location(state, docs_storage, Some(&token)).await;

        assert_eq!(location, "/");
    }
}
