use std::fmt::Write;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, put};
use axum::{Json, Router, middleware};
use kellnr_appstate::{AppStateData, DbState, SettingsState, ToolchainStorageState};
use kellnr_db::{ChannelInfo, ToolchainWithTargets};
use kellnr_web_ui::session::{self, AdminUser};
use serde::{Deserialize, Serialize};
use tracing::trace;

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolchainResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetChannelRequest {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    pub name: String,
    pub version: String,
    pub target: String,
    pub date: String,
    #[serde(default)]
    pub channel: Option<String>,
}

pub fn create_api_routes(_state: AppStateData, max_size: usize) -> Router<AppStateData> {
    Router::new()
        .route("/toolchains", get(list_toolchains))
        .route(
            "/toolchains",
            put(upload_toolchain).layer(DefaultBodyLimit::max(max_size * 1_000_000)),
        )
        .route("/toolchains/{name}/{version}", delete(delete_toolchain))
        .route(
            "/toolchains/{name}/{version}/{target}",
            delete(delete_toolchain_target),
        )
        .route("/channels", get(list_channels))
        .route("/channels/{channel}", put(set_channel))
}

pub fn create_dist_routes(state: AppStateData) -> Router<AppStateData> {
    Router::new()
        // Use a full segment parameter and parse the manifest filename in the handler
        // because Axum doesn't allow parameters in the middle of a path segment
        .route("/{manifest_file}", get(get_channel_manifest))
        .route("/{date}/{filename}", get(download_archive))
        .route_layer(middleware::from_fn_with_state(
            state,
            session::session_auth_when_required,
        ))
}

async fn list_toolchains(
    _user: AdminUser,
    State(db): DbState,
) -> Result<Json<Vec<ToolchainWithTargets>>, StatusCode> {
    trace!("Listing toolchains");
    db.list_toolchains()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn upload_toolchain(
    _user: AdminUser,
    State(db): DbState,
    State(storage): ToolchainStorageState,
    Query(params): Query<UploadQuery>,
    body: axum::body::Bytes,
) -> Result<Json<ToolchainResponse>, (StatusCode, Json<ToolchainResponse>)> {
    trace!(
        name = %params.name,
        version = %params.version,
        target = %params.target,
        "Uploading toolchain"
    );

    let storage = storage.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ToolchainResponse {
                success: false,
                message: Some("Toolchain storage not configured".to_string()),
            }),
        )
    })?;

    // Check if toolchain exists, if not create it
    let toolchain = db
        .get_toolchain_by_version(&params.name, &params.version)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Database error: {e}")),
                }),
            )
        })?;

    let toolchain_id = if let Some(tc) = toolchain {
        // Check if target already exists
        if tc.targets.iter().any(|t| t.target == params.target) {
            return Err((
                StatusCode::CONFLICT,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!(
                        "Target {} already exists for {}-{}",
                        params.target, params.name, params.version
                    )),
                }),
            ));
        }
        tc.id
    } else {
        // Create new toolchain
        db.add_toolchain(
            &params.name,
            &params.version,
            &params.date,
            params.channel.clone(),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Failed to create toolchain: {e}")),
                }),
            )
        })?
    };

    // Get the size before consuming the body
    let size = body.len() as i64;

    // Store the archive
    let (path, hash) = storage
        .put(
            &params.date,
            &params.name,
            &params.version,
            &params.target,
            body,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Failed to store archive: {e}")),
                }),
            )
        })?;

    // Add target to database
    db.add_toolchain_target(toolchain_id, &params.target, &path, &hash, size)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Failed to add target: {e}")),
                }),
            )
        })?;

    Ok(Json(ToolchainResponse {
        success: true,
        message: Some(format!(
            "Uploaded {}-{}-{} ({} bytes)",
            params.name, params.version, params.target, size
        )),
    }))
}

async fn delete_toolchain(
    _user: AdminUser,
    State(db): DbState,
    State(storage): ToolchainStorageState,
    Path((name, version)): Path<(String, String)>,
) -> Result<Json<ToolchainResponse>, (StatusCode, Json<ToolchainResponse>)> {
    trace!(name = %name, version = %version, "Deleting toolchain");

    let storage = storage.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ToolchainResponse {
                success: false,
                message: Some("Toolchain storage not configured".to_string()),
            }),
        )
    })?;

    // Get the toolchain to find all storage paths
    let toolchain = db
        .get_toolchain_by_version(&name, &version)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Database error: {e}")),
                }),
            )
        })?;

    let Some(tc) = toolchain else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ToolchainResponse {
                success: false,
                message: Some(format!("Toolchain {name}-{version} not found")),
            }),
        ));
    };

    // Delete all archives from storage
    for target in &tc.targets {
        if let Err(e) = storage.delete(&target.storage_path).await {
            tracing::warn!("Failed to delete archive from storage: {e}");
        }
    }

    // Delete toolchain from database (cascades to targets)
    db.delete_toolchain(&name, &version).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ToolchainResponse {
                success: false,
                message: Some(format!("Failed to delete toolchain: {e}")),
            }),
        )
    })?;

    Ok(Json(ToolchainResponse {
        success: true,
        message: Some(format!("Deleted {name}-{version} with {} target(s)", tc.targets.len())),
    }))
}

async fn delete_toolchain_target(
    _user: AdminUser,
    State(db): DbState,
    State(storage): ToolchainStorageState,
    Path((name, version, target)): Path<(String, String, String)>,
) -> Result<Json<ToolchainResponse>, (StatusCode, Json<ToolchainResponse>)> {
    trace!(
        name = %name,
        version = %version,
        target = %target,
        "Deleting toolchain target"
    );

    let storage = storage.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ToolchainResponse {
                success: false,
                message: Some("Toolchain storage not configured".to_string()),
            }),
        )
    })?;

    // Get the toolchain to find the storage path and check target count
    let toolchain = db
        .get_toolchain_by_version(&name, &version)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Database error: {e}")),
                }),
            )
        })?;

    let Some(tc) = toolchain else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ToolchainResponse {
                success: false,
                message: Some(format!("Toolchain {name}-{version} not found")),
            }),
        ));
    };

    // Delete the archive from storage
    if let Some(t) = tc.targets.iter().find(|t| t.target == target) {
        if let Err(e) = storage.delete(&t.storage_path).await {
            tracing::warn!("Failed to delete archive from storage: {e}");
        }
    }

    // If this is the last target, delete the entire toolchain
    if tc.targets.len() == 1 {
        db.delete_toolchain(&name, &version).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Failed to delete toolchain: {e}")),
                }),
            )
        })?;

        return Ok(Json(ToolchainResponse {
            success: true,
            message: Some(format!("Deleted {name}-{version} (last target removed)")),
        }));
    }

    // Delete target from database
    db.delete_toolchain_target(&name, &version, &target)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Failed to delete target: {e}")),
                }),
            )
        })?;

    Ok(Json(ToolchainResponse {
        success: true,
        message: Some(format!("Deleted {name}-{version}-{target}")),
    }))
}

async fn list_channels(
    _user: AdminUser,
    State(db): DbState,
) -> Result<Json<Vec<ChannelInfo>>, StatusCode> {
    trace!("Listing channels");
    db.get_channels()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn set_channel(
    _user: AdminUser,
    State(db): DbState,
    Path(channel): Path<String>,
    Json(req): Json<SetChannelRequest>,
) -> Result<Json<ToolchainResponse>, (StatusCode, Json<ToolchainResponse>)> {
    trace!(
        channel = %channel,
        name = %req.name,
        version = %req.version,
        "Setting channel"
    );

    // Verify the toolchain exists
    let toolchain = db
        .get_toolchain_by_version(&req.name, &req.version)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Database error: {e}")),
                }),
            )
        })?;

    if toolchain.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ToolchainResponse {
                success: false,
                message: Some(format!("Toolchain {}-{} not found", req.name, req.version)),
            }),
        ));
    }

    db.set_channel(&channel, &req.name, &req.version)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ToolchainResponse {
                    success: false,
                    message: Some(format!("Failed to set channel: {e}")),
                }),
            )
        })?;

    Ok(Json(ToolchainResponse {
        success: true,
        message: Some(format!(
            "Channel {} now points to {}-{}",
            channel, req.name, req.version
        )),
    }))
}

/// Get the channel manifest (rustup-compatible TOML)
///
/// Expects a filename in the format `channel-rust-{channel}.toml`
/// e.g., `channel-rust-stable.toml` or `channel-rust-nightly.toml`
async fn get_channel_manifest(
    State(db): DbState,
    State(settings): SettingsState,
    Path(manifest_file): Path<String>,
) -> Result<Response, StatusCode> {
    trace!(manifest_file = %manifest_file, "Getting channel manifest");

    // Parse the manifest filename: channel-rust-{channel}.toml
    let channel = manifest_file
        .strip_prefix("channel-rust-")
        .and_then(|s| s.strip_suffix(".toml"))
        .ok_or(StatusCode::NOT_FOUND)?;

    let toolchain = db
        .get_toolchain_by_channel(channel)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let manifest = generate_manifest(&toolchain, &settings);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "text/toml; charset=utf-8".parse().unwrap(),
    );

    Ok((headers, manifest).into_response())
}

async fn download_archive(
    State(storage): ToolchainStorageState,
    Path((date, filename)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    trace!(date = %date, filename = %filename, "Downloading toolchain archive");

    let storage = storage.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let path = format!("{date}/{filename}");
    let archive = storage.get(&path).await.map_err(|e| {
        tracing::warn!("Failed to get toolchain archive {}: {}", path, e);
        StatusCode::NOT_FOUND
    })?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/x-xz".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{filename}\"")
            .parse()
            .unwrap(),
    );

    Ok((headers, Body::from(archive)).into_response())
}

fn generate_manifest(
    toolchain: &ToolchainWithTargets,
    settings: &Arc<kellnr_settings::Settings>,
) -> String {
    let base_url = format!(
        "{}://{}:{}/api/v1/toolchain/dist",
        settings.origin.protocol, settings.origin.hostname, settings.origin.port
    );

    let mut manifest = String::new();
    let _ = write!(
        manifest,
        r#"manifest-version = "2"
date = "{}"
"#,
        toolchain.date
    );

    let _ = write!(
        manifest,
        r#"
[pkg.rust]
version = "{}"
"#,
        toolchain.version
    );

    for target_info in &toolchain.targets {
        let archive_url = format!("{}/{}", base_url, target_info.storage_path);
        let _ = write!(
            manifest,
            r#"
[pkg.rust.target.{}]
available = true
url = "{}"
hash = "{}"
"#,
            target_info.target, archive_url, target_info.hash
        );
    }

    manifest
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use axum::response::Response;
    use axum::routing::{delete, get, put};
    use axum_extra::extract::cookie::Key;
    use bytes::Bytes;
    use cookie::{Cookie, CookieJar};
    use kellnr_appstate::AppStateData;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{DbProvider, ToolchainTargetInfo, ToolchainWithTargets};
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
    use kellnr_storage::toolchain_storage::ToolchainStorage;
    use mockall::predicate::*;
    use serde::de::DeserializeOwned;
    use tempfile::TempDir;
    use tower::ServiceExt;

    use super::*;

    // Fixed test key for encrypting session cookies - must match between cookie creation and app state
    const TEST_KEY: &[u8] = &[1; 64];

    /// Encode cookies with encryption using the test key
    fn encode_cookies<const N: usize, K: Into<Cow<'static, str>>, V: Into<Cow<'static, str>>>(
        cookies: [(K, V); N],
    ) -> String {
        let mut clear = CookieJar::new();
        let mut jar = clear.private_mut(&TEST_KEY.try_into().unwrap());
        for (k, v) in cookies {
            jar.add(Cookie::new(k, v));
        }
        clear
            .iter()
            .map(|c| c.encoded().to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Create an encrypted admin session cookie
    fn admin_cookie() -> String {
        encode_cookies([("kellnr_session_id", "admin_session")])
    }

    /// Create an encrypted non-admin session cookie
    fn non_admin_cookie() -> String {
        encode_cookies([("kellnr_session_id", "user_session")])
    }

    /// Create app state with the given database and toolchain storage
    fn create_app_state(db: Arc<dyn DbProvider>, toolchain_storage: Option<Arc<ToolchainStorage>>) -> AppStateData {
        let settings = kellnr_settings::test_settings();
        let kellnr_storage = Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
        AppStateData {
            db,
            signing_key: Key::from(TEST_KEY),
            crate_storage: Arc::new(KellnrCrateStorage::new(&settings, kellnr_storage)),
            settings: Arc::new(settings),
            toolchain_storage,
            ..kellnr_appstate::test_state()
        }
    }

    /// Create a router with API routes for testing
    fn create_test_router(state: AppStateData) -> Router {
        let api_routes = Router::new()
            .route("/toolchains", get(list_toolchains))
            .route("/toolchains", put(upload_toolchain))
            .route(
                "/toolchains/{name}/{version}",
                delete(delete_toolchain),
            )
            .route(
                "/toolchains/{name}/{version}/{target}",
                delete(delete_toolchain_target),
            )
            .route("/channels", get(list_channels))
            .route("/channels/{channel}", put(set_channel));

        let dist_routes = Router::new()
            .route("/{manifest_file}", get(get_channel_manifest))
            .route("/{date}/{filename}", get(download_archive));

        Router::new()
            .nest("/api/v1/toolchain", api_routes)
            .nest("/api/v1/toolchain/dist", dist_routes)
            .with_state(state)
    }

    async fn parse_response<T: DeserializeOwned>(response: Response<Body>) -> T {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn sample_archive() -> Bytes {
        // Create a minimal valid-looking archive (XZ magic bytes)
        Bytes::from(vec![0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00])
    }

    // ==================== Authorization Tests ====================

    #[tokio::test]
    async fn test_list_toolchains_admin_authorized() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db
            .expect_list_toolchains()
            .returning(|| Ok(vec![]));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/toolchains")
                    .header(header::COOKIE, admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());
    }

    #[tokio::test]
    async fn test_list_toolchains_non_admin_forbidden() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/toolchains")
                    .header(header::COOKIE, non_admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::FORBIDDEN, response.status());
    }

    #[tokio::test]
    async fn test_list_toolchains_no_cookie_unauthorized() {
        let mock_db = MockDb::new();

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/toolchains")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::UNAUTHORIZED, response.status());
    }

    #[tokio::test]
    async fn test_upload_toolchain_non_admin_forbidden() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::put(
                    "/api/v1/toolchain/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15",
                )
                .header(header::COOKIE, non_admin_cookie())
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(sample_archive()))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::FORBIDDEN, response.status());
    }

    #[tokio::test]
    async fn test_delete_toolchain_non_admin_forbidden() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::delete("/api/v1/toolchain/toolchains/rust/1.0.0/x86_64-unknown-linux-gnu")
                    .header(header::COOKIE, non_admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::FORBIDDEN, response.status());
    }

    #[tokio::test]
    async fn test_list_channels_non_admin_forbidden() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/channels")
                    .header(header::COOKIE, non_admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::FORBIDDEN, response.status());
    }

    #[tokio::test]
    async fn test_set_channel_non_admin_forbidden() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let payload = r#"{"name": "rust", "version": "1.0.0"}"#;

        let response = router
            .oneshot(
                Request::put("/api/v1/toolchain/channels/stable")
                    .header(header::COOKIE, non_admin_cookie())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::FORBIDDEN, response.status());
    }

    // ==================== Happy Path Tests ====================

    #[tokio::test]
    async fn test_list_toolchains_returns_data() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db.expect_list_toolchains().returning(|| {
            Ok(vec![ToolchainWithTargets {
                id: 1,
                name: "rust".to_string(),
                version: "1.0.0".to_string(),
                date: "2024-01-15".to_string(),
                channel: Some("stable".to_string()),
                created: "2024-01-15".to_string(),
                targets: vec![ToolchainTargetInfo {
                    id: 1,
                    target: "x86_64-unknown-linux-gnu".to_string(),
                    storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz".to_string(),
                    hash: "abc123".to_string(),
                    size: 1024,
                }],
            }])
        });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/toolchains")
                    .header(header::COOKIE, admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let toolchains: Vec<ToolchainWithTargets> = parse_response(response).await;
        assert_eq!(toolchains.len(), 1);
        assert_eq!(toolchains[0].name, "rust");
        assert_eq!(toolchains[0].version, "1.0.0");
        assert_eq!(toolchains[0].targets.len(), 1);
    }

    #[tokio::test]
    async fn test_list_channels_returns_data() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db.expect_get_channels().returning(|| {
            Ok(vec![ChannelInfo {
                name: "stable".to_string(),
                version: "1.0.0".to_string(),
                date: "2024-01-15".to_string(),
            }])
        });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/channels")
                    .header(header::COOKIE, admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let channels: Vec<ChannelInfo> = parse_response(response).await;
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "stable");
        assert_eq!(channels[0].version, "1.0.0");
    }

    #[tokio::test]
    async fn test_upload_toolchain_success() {
        let temp_dir = TempDir::new().unwrap();
        let storage: DynStorage = Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| Ok(None));
        mock_db
            .expect_add_toolchain()
            .with(eq("rust"), eq("1.0.0"), eq("2024-01-15"), eq(Some("stable".to_string())))
            .returning(|_, _, _, _| Ok(1));
        mock_db
            .expect_add_toolchain_target()
            .returning(|_, _, _, _, _| Ok(()));

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::put(
                    "/api/v1/toolchain/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15&channel=stable",
                )
                .header(header::COOKIE, admin_cookie())
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(sample_archive()))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
        assert!(result.message.unwrap().contains("Uploaded"));
    }

    #[tokio::test]
    async fn test_set_channel_success() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| {
                Ok(Some(ToolchainWithTargets {
                    id: 1,
                    name: "rust".to_string(),
                    version: "1.0.0".to_string(),
                    date: "2024-01-15".to_string(),
                    channel: None,
                    created: "2024-01-15".to_string(),
                    targets: vec![],
                }))
            });
        mock_db
            .expect_set_channel()
            .with(eq("stable"), eq("rust"), eq("1.0.0"))
            .returning(|_, _, _| Ok(()));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let payload = r#"{"name": "rust", "version": "1.0.0"}"#;

        let response = router
            .oneshot(
                Request::put("/api/v1/toolchain/channels/stable")
                    .header(header::COOKIE, admin_cookie())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_delete_toolchain_target_success() {
        let temp_dir = TempDir::new().unwrap();
        let storage: DynStorage = Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| {
                // Two targets so deleting one doesn't trigger auto-delete of toolchain
                Ok(Some(ToolchainWithTargets {
                    id: 1,
                    name: "rust".to_string(),
                    version: "1.0.0".to_string(),
                    date: "2024-01-15".to_string(),
                    channel: None,
                    created: "2024-01-15".to_string(),
                    targets: vec![
                        ToolchainTargetInfo {
                            id: 1,
                            target: "x86_64-unknown-linux-gnu".to_string(),
                            storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz".to_string(),
                            hash: "abc123".to_string(),
                            size: 1024,
                        },
                        ToolchainTargetInfo {
                            id: 2,
                            target: "aarch64-unknown-linux-gnu".to_string(),
                            storage_path: "2024-01-15/rust-1.0.0-aarch64-unknown-linux-gnu.tar.xz".to_string(),
                            hash: "def456".to_string(),
                            size: 2048,
                        },
                    ],
                }))
            });
        mock_db
            .expect_delete_toolchain_target()
            .with(eq("rust"), eq("1.0.0"), eq("x86_64-unknown-linux-gnu"))
            .returning(|_, _, _| Ok(()));

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::delete("/api/v1/toolchain/toolchains/rust/1.0.0/x86_64-unknown-linux-gnu")
                    .header(header::COOKIE, admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_delete_last_target_auto_deletes_toolchain() {
        let temp_dir = TempDir::new().unwrap();
        let storage: DynStorage = Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| {
                // Only one target - deleting it should auto-delete the toolchain
                Ok(Some(ToolchainWithTargets {
                    id: 1,
                    name: "rust".to_string(),
                    version: "1.0.0".to_string(),
                    date: "2024-01-15".to_string(),
                    channel: None,
                    created: "2024-01-15".to_string(),
                    targets: vec![ToolchainTargetInfo {
                        id: 1,
                        target: "x86_64-unknown-linux-gnu".to_string(),
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz".to_string(),
                        hash: "abc123".to_string(),
                        size: 1024,
                    }],
                }))
            });
        // Expect delete_toolchain to be called (not delete_toolchain_target)
        // because this is the last target
        mock_db
            .expect_delete_toolchain()
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| Ok(()));

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::delete("/api/v1/toolchain/toolchains/rust/1.0.0/x86_64-unknown-linux-gnu")
                    .header(header::COOKIE, admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
        // Message should indicate the whole toolchain was deleted (last target removed)
        let msg = result.message.unwrap();
        assert!(msg.contains("last target removed"), "Expected 'last target removed' in message: {}", msg);
    }

    #[tokio::test]
    async fn test_delete_toolchain_success() {
        let temp_dir = TempDir::new().unwrap();
        let storage: DynStorage = Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| {
                Ok(Some(ToolchainWithTargets {
                    id: 1,
                    name: "rust".to_string(),
                    version: "1.0.0".to_string(),
                    date: "2024-01-15".to_string(),
                    channel: Some("stable".to_string()),
                    created: "2024-01-15".to_string(),
                    targets: vec![
                        ToolchainTargetInfo {
                            id: 1,
                            target: "x86_64-unknown-linux-gnu".to_string(),
                            storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz".to_string(),
                            hash: "abc123".to_string(),
                            size: 1024,
                        },
                        ToolchainTargetInfo {
                            id: 2,
                            target: "aarch64-unknown-linux-gnu".to_string(),
                            storage_path: "2024-01-15/rust-1.0.0-aarch64-unknown-linux-gnu.tar.xz".to_string(),
                            hash: "def456".to_string(),
                            size: 2048,
                        },
                    ],
                }))
            });
        mock_db
            .expect_delete_toolchain()
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| Ok(()));

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::delete("/api/v1/toolchain/toolchains/rust/1.0.0")
                    .header(header::COOKIE, admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
        assert!(result.message.unwrap().contains("Deleted"));
    }

    #[tokio::test]
    async fn test_delete_toolchain_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let storage: DynStorage = Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("99.99.99"))
            .returning(|_, _| Ok(None));

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::delete("/api/v1/toolchain/toolchains/rust/99.99.99")
                    .header(header::COOKIE, admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(!result.success);
        assert!(result.message.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_delete_entire_toolchain_non_admin_forbidden() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::delete("/api/v1/toolchain/toolchains/rust/1.0.0")
                    .header(header::COOKIE, non_admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::FORBIDDEN, response.status());
    }

    // ==================== Error Path Tests ====================

    #[tokio::test]
    async fn test_upload_toolchain_storage_not_configured() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));

        // No toolchain storage configured
        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::put(
                    "/api/v1/toolchain/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15",
                )
                .header(header::COOKIE, admin_cookie())
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(sample_archive()))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::SERVICE_UNAVAILABLE, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(!result.success);
        assert!(result.message.unwrap().contains("not configured"));
    }

    #[tokio::test]
    async fn test_upload_duplicate_target_conflict() {
        let temp_dir = TempDir::new().unwrap();
        let storage: DynStorage = Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| {
                Ok(Some(ToolchainWithTargets {
                    id: 1,
                    name: "rust".to_string(),
                    version: "1.0.0".to_string(),
                    date: "2024-01-15".to_string(),
                    channel: None,
                    created: "2024-01-15".to_string(),
                    targets: vec![ToolchainTargetInfo {
                        id: 1,
                        target: "x86_64-unknown-linux-gnu".to_string(),
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz".to_string(),
                        hash: "abc123".to_string(),
                        size: 1024,
                    }],
                }))
            });

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::put(
                    "/api/v1/toolchain/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15",
                )
                .header(header::COOKIE, admin_cookie())
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(sample_archive()))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::CONFLICT, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(!result.success);
        assert!(result.message.unwrap().contains("already exists"));
    }

    #[tokio::test]
    async fn test_set_channel_toolchain_not_found() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("99.99.99"))
            .returning(|_, _| Ok(None));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let payload = r#"{"name": "rust", "version": "99.99.99"}"#;

        let response = router
            .oneshot(
                Request::put("/api/v1/toolchain/channels/stable")
                    .header(header::COOKIE, admin_cookie())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(!result.success);
        assert!(result.message.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_get_manifest_invalid_format_not_found() {
        let mock_db = MockDb::new();

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        // Missing prefix
        let response = router
            .clone()
            .oneshot(
                Request::get("/api/v1/toolchain/dist/stable.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, response.status());
    }

    #[tokio::test]
    async fn test_get_manifest_channel_not_found() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_toolchain_by_channel()
            .with(eq("nonexistent"))
            .returning(|_| Ok(None));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/dist/channel-rust-nonexistent.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, response.status());
    }

    #[tokio::test]
    async fn test_download_archive_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let storage: DynStorage = Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

        let mock_db = MockDb::new();

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/dist/2024-01-15/nonexistent.tar.xz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, response.status());
    }

    #[tokio::test]
    async fn test_download_archive_storage_not_configured() {
        let mock_db = MockDb::new();

        // No toolchain storage
        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/dist/2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::SERVICE_UNAVAILABLE, response.status());
    }

    // ==================== Manifest Generation Test ====================

    #[tokio::test]
    async fn test_get_manifest_returns_valid_toml() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_toolchain_by_channel()
            .with(eq("stable"))
            .returning(|_| {
                Ok(Some(ToolchainWithTargets {
                    id: 1,
                    name: "rust".to_string(),
                    version: "1.0.0".to_string(),
                    date: "2024-01-15".to_string(),
                    channel: Some("stable".to_string()),
                    created: "2024-01-15".to_string(),
                    targets: vec![ToolchainTargetInfo {
                        id: 1,
                        target: "x86_64-unknown-linux-gnu".to_string(),
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz".to_string(),
                        hash: "abc123def456".to_string(),
                        size: 1024,
                    }],
                }))
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchain/dist/channel-rust-stable.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/toml; charset=utf-8"
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let manifest = String::from_utf8(body.to_vec()).unwrap();

        // Verify manifest structure
        assert!(manifest.contains("manifest-version = \"2\""));
        assert!(manifest.contains("date = \"2024-01-15\""));
        assert!(manifest.contains("[pkg.rust]"));
        assert!(manifest.contains("version = \"1.0.0\""));
        assert!(manifest.contains("[pkg.rust.target.x86_64-unknown-linux-gnu]"));
        assert!(manifest.contains("available = true"));
        assert!(manifest.contains("url = \""));
        assert!(manifest.contains("hash = \"abc123def456\""));
    }
}
