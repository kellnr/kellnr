use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Read as _, Seek as _, Write};
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use kellnr_common::background_tasks::BackgroundTasks;
use kellnr_common::binary_body::BinaryBody;
use kellnr_db::error::DbError;
use kellnr_db::{ChannelInfo, ToolchainTargetInfo, ToolchainTargetStatus, ToolchainWithTargets};
use kellnr_http::error::{ApiError, ApiResult};
use kellnr_http::identity::AdminUser;
use kellnr_http::state::{BackgroundTasksState, DbState, SettingsState, ToolchainStorageState};
use kellnr_storage::storage_error::StorageError;
use kellnr_storage::storage_key::StorageKey;
use kellnr_storage::toolchain_storage::{ToolchainArchiveId, ToolchainStorage};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tracing::trace;
use utoipa::ToSchema;

/// Response for toolchain operations
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ToolchainResponse {
    /// Whether the operation succeeded
    pub success: bool,
    /// Optional message with details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Request to set a channel's toolchain
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SetChannelRequest {
    /// Toolchain name (e.g., "rust")
    pub name: String,
    /// Toolchain version (e.g., "1.75.0")
    pub version: String,
}

/// Query parameters for uploading a toolchain
#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct UploadQuery {
    /// Toolchain name (e.g., "rust")
    pub name: String,
    /// Toolchain version (e.g., "1.75.0")
    pub version: String,
    /// Target triple (e.g., "x86_64-unknown-linux-gnu")
    pub target: String,
    /// Release date (e.g., "2024-01-15")
    pub date: String,
    /// Optional channel to associate (e.g., "stable")
    #[serde(default)]
    pub channel: Option<String>,
}

/// Logs the error and answers with a 500 that does not reveal its details.
fn internal_error(message: &str, error: impl std::fmt::Display) -> ApiError {
    tracing::error!("{message}: {error}");
    ApiError::new(message, StatusCode::INTERNAL_SERVER_ERROR)
}

fn storage_not_configured() -> ApiError {
    ApiError::new(
        "Toolchain storage not configured",
        StatusCode::SERVICE_UNAVAILABLE,
    )
}

/// List all toolchains
///
/// Returns all toolchains with their available targets.
/// Requires admin access.
#[utoipa::path(
    get,
    path = "/",
    tag = "toolchains",
    responses(
        (status = 200, description = "List of toolchains", body = Vec<ToolchainWithTargets>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_toolchains(
    _user: AdminUser,
    State(db): DbState,
) -> ApiResult<Json<Vec<ToolchainWithTargets>>> {
    trace!("Listing toolchains");
    db.list_toolchains()
        .await
        .map(Json)
        .map_err(|e| internal_error("Failed to load toolchains", e))
}

/// Upload a toolchain
///
/// Adds the combined archive of one target to a toolchain. The archive is
/// split into its components in the background.
/// Requires admin access.
#[utoipa::path(
    put,
    path = "/",
    tag = "toolchains",
    params(UploadQuery),
    request_body(content = inline(BinaryBody), content_type = "application/octet-stream", description = "Combined toolchain archive (.tar.xz)"),
    responses(
        (status = 200, description = "Toolchain uploaded", body = ToolchainResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Admin access required"),
        (status = 409, description = "Target or archive already exists"),
        (status = 413, description = "Archive is too large"),
        (status = 503, description = "Storage not configured")
    ),
    security(("session_cookie" = []))
)]
pub async fn upload_toolchain(
    _user: AdminUser,
    State(db): DbState,
    State(storage): ToolchainStorageState,
    State(tasks): BackgroundTasksState,
    Query(params): Query<UploadQuery>,
    body: Bytes,
) -> ApiResult<Json<ToolchainResponse>> {
    trace!(
        name = %params.name,
        version = %params.version,
        target = %params.target,
        "Uploading toolchain"
    );

    let storage = storage.as_ref().ok_or_else(storage_not_configured)?;

    let toolchain = db
        .get_toolchain_by_version(&params.name, &params.version)
        .await
        .map_err(|e| internal_error("Failed to load toolchain", e))?;

    if let Some(tc) = &toolchain
        && tc.targets.iter().any(|t| t.target == params.target)
    {
        return Err(ApiError::conflict(format!(
            "Target {} already exists for {}-{}",
            params.target, params.name, params.version
        )));
    }

    let size = body.len() as i64;

    // Store the archive before it is added to the database, so a failed store
    // does not leave a toolchain or target without archive behind.
    let archive_id = ToolchainArchiveId {
        name: &params.name,
        version: &params.version,
        date: &params.date,
        target: &params.target,
    };
    let (path, hash) = storage.put(&archive_id, body).await.map_err(|e| match e {
        StorageError::ToolchainArchiveExists { .. } => ApiError::conflict(e.to_string()),
        e => internal_error("Failed to store archive", e),
    })?;

    let target_id =
        match add_target_to_db(&db, toolchain.as_ref(), &params, &path, &hash, size).await {
            Ok(id) => id,
            Err(e) => {
                if let Err(e) = storage.delete(&path).await {
                    tracing::warn!("Failed to delete archive {path} after failed upload: {e}");
                }
                return Err(e);
            }
        };

    // Spawn background task to split the combined archive into individual component archives.
    // This can take a while for large archives, so we don't block the upload response.
    // The target status is "processing" until extraction completes; the manifest only
    // includes targets with status "ready".
    let bg_storage = storage.clone();
    let bg_db = db.clone();
    let bg_name = params.name.clone();
    let bg_version = params.version.clone();
    let bg_target = params.target.clone();
    let bg_date = params.date.clone();
    let bg_path = path.clone();
    spawn_target_processing(&tasks, db.clone(), target_id, async move {
        let archive_bytes = {
            let mut archive = None;
            for attempt in 1..=3 {
                match bg_storage.get(&bg_path).await {
                    Ok(bytes) => {
                        archive = Some(bytes);
                        break;
                    }
                    Err(e) => {
                        if attempt == 3 {
                            tracing::warn!(
                                "Failed to read archive for component extraction after {attempt} attempts: {e}"
                            );
                            if let Err(status_err) = bg_db
                                .set_target_status(target_id, ToolchainTargetStatus::Failed)
                                .await
                            {
                                tracing::warn!(
                                    "Failed to set target status to failed after archive read error: {status_err}"
                                );
                            }
                            return;
                        }
                        tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
                    }
                }
            }
            let Some(bytes) = archive else {
                tracing::warn!("Archive read retry loop finished without archive bytes");
                return;
            };
            bytes
        };

        let archive_id = ToolchainArchiveId {
            name: &bg_name,
            version: &bg_version,
            date: &bg_date,
            target: &bg_target,
        };
        match extract_and_store_components(
            &bg_storage,
            &bg_db,
            target_id,
            &archive_id,
            archive_bytes,
        )
        .await
        {
            Ok(()) => {
                if let Err(e) = bg_db
                    .set_target_status(target_id, ToolchainTargetStatus::Ready)
                    .await
                {
                    tracing::warn!("Failed to set target status to ready: {e}");
                }
                tracing::info!(
                    "Successfully extracted components for {}-{}-{}",
                    bg_name,
                    bg_version,
                    bg_target
                );
            }
            Err(e) => {
                if let Err(status_err) = bg_db
                    .set_target_status(target_id, ToolchainTargetStatus::Failed)
                    .await
                {
                    tracing::warn!("Failed to set target status to failed: {status_err}");
                }
                tracing::warn!("Failed to extract components from archive: {e}");
            }
        }
    });

    Ok(Json(ToolchainResponse {
        success: true,
        message: Some(format!(
            "Uploaded {}-{}-{} ({} bytes); components are being processed",
            params.name, params.version, params.target, size
        )),
    }))
}

/// Runs the processing of an uploaded target in the background. If it panics,
/// the target is marked as failed instead of staying "processing" forever.
fn spawn_target_processing(
    tasks: &BackgroundTasks,
    db: Arc<dyn kellnr_db::DbProvider>,
    target_id: i64,
    processing: impl Future<Output = ()> + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tasks.spawn_tracked(async move {
        if let Err(e) = tokio::spawn(processing).await {
            tracing::error!("Processing of toolchain target {target_id} stopped: {e}");
            if let Err(e) = db
                .set_target_status(target_id, ToolchainTargetStatus::Failed)
                .await
            {
                tracing::warn!("Failed to set target status to failed: {e}");
            }
        }
    })
}

/// Adds the uploaded target, and its toolchain if it is new, to the database.
/// Removes a toolchain created here again if the target cannot be added.
async fn add_target_to_db(
    db: &Arc<dyn kellnr_db::DbProvider>,
    toolchain: Option<&ToolchainWithTargets>,
    params: &UploadQuery,
    path: &StorageKey,
    hash: &str,
    size: i64,
) -> ApiResult<i64> {
    let toolchain_id = match toolchain {
        Some(tc) => tc.id,
        None => db
            .add_toolchain(
                &params.name,
                &params.version,
                &params.date,
                params.channel.clone(),
            )
            .await
            .map_err(|e| internal_error("Failed to create toolchain", e))?,
    };

    match db
        .add_toolchain_target(toolchain_id, &params.target, path.as_str(), hash, size)
        .await
    {
        Ok(target_id) => Ok(target_id),
        Err(e) => {
            if toolchain.is_none()
                && let Err(e) = db.delete_toolchain(&params.name, &params.version).await
            {
                tracing::warn!(
                    "Failed to delete toolchain {}-{} after failed upload: {e}",
                    params.name,
                    params.version
                );
            }
            Err(internal_error("Failed to add target", e))
        }
    }
}

/// Delete a toolchain and all its targets
///
/// Removes a toolchain version and all associated target archives.
/// Requires admin access.
#[utoipa::path(
    delete,
    path = "/{name}/{version}",
    tag = "toolchains",
    params(
        ("name" = String, Path, description = "Toolchain name"),
        ("version" = String, Path, description = "Toolchain version")
    ),
    responses(
        (status = 200, description = "Toolchain deleted", body = ToolchainResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Admin access required"),
        (status = 404, description = "Toolchain not found"),
        (status = 503, description = "Storage not configured")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_toolchain(
    _user: AdminUser,
    State(db): DbState,
    State(storage): ToolchainStorageState,
    Path((name, version)): Path<(String, String)>,
) -> ApiResult<Json<ToolchainResponse>> {
    trace!(name = %name, version = %version, "Deleting toolchain");

    let storage = storage.as_ref().ok_or_else(storage_not_configured)?;

    let toolchain = db
        .get_toolchain_by_version(&name, &version)
        .await
        .map_err(|e| internal_error("Failed to load toolchain", e))?;

    let Some(tc) = toolchain else {
        return Err(ApiError::not_found(format!(
            "Toolchain {name}-{version} not found"
        )));
    };

    // Delete from the database first (cascades to targets and components), so
    // a failure does not leave targets whose archives are gone.
    db.delete_toolchain(&name, &version)
        .await
        .map_err(|e| internal_error("Failed to delete toolchain", e))?;

    for target in &tc.targets {
        delete_target_archives(storage, target).await;
    }

    Ok(Json(ToolchainResponse {
        success: true,
        message: Some(format!(
            "Deleted {name}-{version} with {} target(s)",
            tc.targets.len()
        )),
    }))
}

/// Delete a specific toolchain target
///
/// Removes a specific target from a toolchain. If this is the last target,
/// the entire toolchain is deleted.
/// Requires admin access.
#[utoipa::path(
    delete,
    path = "/{name}/{version}/targets/{target}",
    tag = "toolchains",
    params(
        ("name" = String, Path, description = "Toolchain name"),
        ("version" = String, Path, description = "Toolchain version"),
        ("target" = String, Path, description = "Target triple")
    ),
    responses(
        (status = 200, description = "Target deleted", body = ToolchainResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Admin access required"),
        (status = 404, description = "Toolchain or target not found"),
        (status = 503, description = "Storage not configured")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_toolchain_target(
    _user: AdminUser,
    State(db): DbState,
    State(storage): ToolchainStorageState,
    Path((name, version, target)): Path<(String, String, String)>,
) -> ApiResult<Json<ToolchainResponse>> {
    trace!(
        name = %name,
        version = %version,
        target = %target,
        "Deleting toolchain target"
    );

    let storage = storage.as_ref().ok_or_else(storage_not_configured)?;

    let toolchain = db
        .get_toolchain_by_version(&name, &version)
        .await
        .map_err(|e| internal_error("Failed to load toolchain", e))?;

    let Some(tc) = toolchain else {
        return Err(ApiError::not_found(format!(
            "Toolchain {name}-{version} not found"
        )));
    };

    let Some(t) = tc.targets.iter().find(|t| t.target == target) else {
        return Err(ApiError::not_found(format!(
            "Target {target} of {name}-{version} not found"
        )));
    };

    // Delete from the database first, so a failure does not leave a target
    // whose archives are gone. If this is the last target, the entire
    // toolchain is deleted.
    let last_target = tc.targets.len() == 1;
    if last_target {
        db.delete_toolchain(&name, &version)
            .await
            .map_err(|e| internal_error("Failed to delete toolchain", e))?;
    } else {
        db.delete_toolchain_target(&name, &version, &target)
            .await
            .map_err(|e| internal_error("Failed to delete target", e))?;
    }

    delete_target_archives(storage, t).await;

    if last_target {
        return Ok(Json(ToolchainResponse {
            success: true,
            message: Some(format!("Deleted {name}-{version} (last target removed)")),
        }));
    }

    Ok(Json(ToolchainResponse {
        success: true,
        message: Some(format!("Deleted {name}-{version}-{target}")),
    }))
}

/// Deletes the archive of a target and its component archives. Failures are
/// only logged, the target is already removed from the database.
async fn delete_target_archives(storage: &ToolchainStorage, target: &ToolchainTargetInfo) {
    if let Err(e) = storage
        .delete(&StorageKey::from_stored(target.storage_path.clone()))
        .await
    {
        tracing::warn!("Failed to delete archive from storage: {e}");
    }
    for component in &target.components {
        if let Err(e) = storage
            .delete(&StorageKey::from_stored(component.storage_path.clone()))
            .await
        {
            tracing::warn!("Failed to delete component archive from storage: {e}");
        }
    }
}

/// List all channels
///
/// Returns all configured channels (e.g., stable, beta, nightly) and
/// the toolchain versions they point to.
/// Requires admin access.
#[utoipa::path(
    get,
    path = "/channels",
    tag = "toolchains",
    responses(
        (status = 200, description = "List of channels", body = Vec<ChannelInfo>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_channels(
    _user: AdminUser,
    State(db): DbState,
) -> ApiResult<Json<Vec<ChannelInfo>>> {
    trace!("Listing channels");
    db.get_channels()
        .await
        .map(Json)
        .map_err(|e| internal_error("Failed to load channels", e))
}

/// Set a channel to point to a specific toolchain version
///
/// Updates a channel (e.g., "stable") to point to a specific toolchain version.
/// Requires admin access.
#[utoipa::path(
    put,
    path = "/channels/{channel}",
    tag = "toolchains",
    params(
        ("channel" = String, Path, description = "Channel name (e.g., stable, beta, nightly)")
    ),
    request_body = SetChannelRequest,
    responses(
        (status = 200, description = "Channel updated", body = ToolchainResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Admin access required"),
        (status = 404, description = "Toolchain not found")
    ),
    security(("session_cookie" = []))
)]
pub async fn set_channel(
    _user: AdminUser,
    State(db): DbState,
    Path(channel): Path<String>,
    Json(req): Json<SetChannelRequest>,
) -> ApiResult<Json<ToolchainResponse>> {
    trace!(
        channel = %channel,
        name = %req.name,
        version = %req.version,
        "Setting channel"
    );

    db.set_channel(&channel, &req.name, &req.version)
        .await
        .map_err(|e| match e {
            DbError::ToolchainNotFound(..) => ApiError::not_found(e.to_string()),
            e => internal_error("Failed to set channel", e),
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
/// Returns a rustup-compatible manifest for a channel.
/// Expects a filename in the format `channel-rust-{channel}.toml`
/// e.g., `channel-rust-stable.toml` or `channel-rust-nightly.toml`
#[utoipa::path(
    get,
    path = "/{manifest_file}",
    tag = "toolchains",
    params(
        ("manifest_file" = String, Path, description = "Manifest filename (e.g., channel-rust-stable.toml)")
    ),
    responses(
        (status = 200, description = "Channel manifest (TOML)", content_type = "text/toml"),
        (status = 404, description = "Channel not found or no targets available"),
        (status = 503, description = "Targets are still being processed")
    ),
    security(("session_cookie" = []))
)]
pub async fn get_channel_manifest(
    State(db): DbState,
    State(settings): SettingsState,
    Path(manifest_file): Path<String>,
) -> ApiResult<Response> {
    trace!(manifest_file = %manifest_file, "Getting channel manifest");

    // Parse the manifest filename:
    //   channel-rust-{channel}.toml         -> return the manifest
    //   channel-rust-{channel}.toml.sha256  -> return the SHA256 hash of the manifest
    let (channel, want_sha256) = manifest_file
        .strip_prefix("channel-rust-")
        .and_then(|s| {
            s.strip_suffix(".toml.sha256")
                .map(|c| (c, true))
                .or_else(|| s.strip_suffix(".toml").map(|c| (c, false)))
        })
        .ok_or(StatusCode::NOT_FOUND)?;

    // Try channel lookup first (e.g. "stable"), then fall back to version lookup
    // (e.g. "1.94.0") so that `rustup install 1.94.0` works too.
    let toolchain = match db
        .get_toolchain_by_channel(channel)
        .await
        .map_err(|e| internal_error("Failed to load toolchains", e))?
    {
        Some(tc) => tc,
        None => db
            .get_toolchain_by_version("rust", channel)
            .await
            .map_err(|e| internal_error("Failed to load toolchains", e))?
            .ok_or(StatusCode::NOT_FOUND)?,
    };

    // Check if any targets are ready before generating the manifest.
    // If none are ready, the manifest would be invalid (missing required `target` field).
    let has_ready_targets = toolchain
        .targets
        .iter()
        .any(|t| t.status == ToolchainTargetStatus::Ready);
    if !has_ready_targets {
        let has_processing = toolchain
            .targets
            .iter()
            .any(|t| t.status == ToolchainTargetStatus::Processing);
        if has_processing {
            return Err(StatusCode::SERVICE_UNAVAILABLE.into());
        }
        return Err(StatusCode::NOT_FOUND.into());
    }

    let manifest = generate_manifest(&toolchain, &settings);

    if want_sha256 {
        let hash = sha256::digest(manifest.as_bytes());
        Ok(hash.into_response())
    } else {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            "text/toml; charset=utf-8".parse().unwrap(),
        );
        Ok((headers, manifest).into_response())
    }
}

/// Download a toolchain archive
///
/// Downloads a specific toolchain archive file.
#[utoipa::path(
    get,
    path = "/{date}/{filename}",
    tag = "toolchains",
    params(
        ("date" = String, Path, description = "Release date (e.g., 2024-01-15)"),
        ("filename" = String, Path, description = "Archive filename")
    ),
    responses(
        (status = 200, description = "Toolchain archive (xz, gzip, or raw)", content_type = "application/octet-stream"),
        (status = 404, description = "Archive not found"),
        (status = 503, description = "Storage not configured")
    ),
    security(("session_cookie" = []))
)]
pub async fn download_archive(
    State(storage): ToolchainStorageState,
    Path((date, filename)): Path<(String, String)>,
) -> ApiResult<Response> {
    trace!(date = %date, filename = %filename, "Downloading toolchain archive");

    let storage = storage.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let path = StorageKey::toolchain_file(&date, &filename);
    let archive = storage.get(&path).await.map_err(|e| {
        tracing::warn!("Failed to get toolchain archive {}: {}", path, e);
        StatusCode::NOT_FOUND
    })?;

    let path_ref = std::path::Path::new(&filename);
    let ext = path_ref.extension().and_then(|e| e.to_str()).unwrap_or("");
    let content_type = if ext.eq_ignore_ascii_case("xz") {
        "application/x-xz"
    } else if ext.eq_ignore_ascii_case("gz") {
        "application/gzip"
    } else {
        "application/octet-stream"
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{filename}\"")
            .parse()
            .unwrap(),
    );

    Ok((headers, Body::from(archive)).into_response())
}

/// Limits how many uploaded archives are split into components at the same
/// time. Splitting decompresses a whole toolchain and compresses every
/// component again, which keeps a CPU core busy for minutes.
static EXTRACTION_PERMITS: Semaphore = Semaphore::const_new(1);

/// Extract individual component archives from a combined toolchain archive.
///
/// The combined `rust-{version}-{target}.tar.xz` archive contains sub-component
/// directories (rustc, cargo, rust-std, etc.) each with their own `manifest.in`.
/// This function extracts each component into its own `.tar.xz` archive so rustup
/// can download them individually.
///
/// The decompressed components are written to temporary files and compressed
/// one by one, so only the archive and one compressed component are held in
/// memory. The CPU heavy work runs on the blocking thread pool.
async fn extract_and_store_components(
    storage: &Arc<ToolchainStorage>,
    db: &Arc<dyn kellnr_db::DbProvider>,
    target_id: i64,
    archive_id: &ToolchainArchiveId<'_>,
    archive_bytes: Bytes,
) -> Result<(), String> {
    let _permit = EXTRACTION_PERMITS
        .acquire()
        .await
        .map_err(|e| format!("Failed to wait for component extraction: {e}"))?;

    let components = {
        let (name, version, target) = (
            archive_id.name.to_string(),
            archive_id.version.to_string(),
            archive_id.target.to_string(),
        );
        tokio::task::spawn_blocking(move || {
            split_components(&archive_bytes, &name, &version, &target)
        })
        .await
        .map_err(|e| format!("Component extraction stopped: {e}"))??
    };

    tracing::debug!(
        "Extracted {} components in two streamed passes, compressing and storing",
        components.len()
    );

    // Compress and store each component archive.
    // Track stored paths so we can clean up on failure (e.g. if the target
    // was deleted concurrently and the DB insert fails with an FK violation).
    let mut stored_paths: Vec<StorageKey> = Vec::new();

    for (component_name, component_tar) in components {
        let component_xz = tokio::task::spawn_blocking(move || compress_component(component_tar))
            .await
            .map_err(|e| format!("Compression of {component_name} stopped: {e}"))?
            .map_err(|e| format!("Failed to compress {component_name}: {e}"))?;

        let component_hash = sha256::digest(&component_xz[..]);
        let component_size = component_xz.len() as i64;
        let component_storage_path = StorageKey::toolchain_component(archive_id, &component_name);

        // A file no target refers to is left over, e.g. from a target whose
        // archives could not all be deleted, and would block the target.
        let in_use = db
            .is_toolchain_component_stored_at(component_storage_path.as_str())
            .await
            .map_err(|e| format!("Failed to check component {component_name}: {e}"))?;
        let component_xz = Bytes::from(component_xz);
        let stored = if in_use {
            storage.put_raw(&component_storage_path, component_xz).await
        } else {
            storage
                .put_raw_overwrite(&component_storage_path, component_xz)
                .await
        };
        stored.map_err(|e| format!("Failed to store component {component_name}: {e}"))?;
        stored_paths.push(component_storage_path.clone());

        if let Err(e) = db
            .add_toolchain_component(
                target_id,
                &component_name,
                component_storage_path.as_str(),
                &component_hash,
                component_size,
            )
            .await
        {
            // DB insert failed (likely FK violation from concurrent delete).
            // Clean up all stored component files to avoid orphans.
            for p in &stored_paths {
                let _ = storage.delete(p).await;
            }
            return Err(format!(
                "Failed to add component {component_name} to DB: {e}"
            ));
        }

        tracing::debug!(
            component = component_name,
            path = component_storage_path.as_str(),
            size = component_size,
            "Stored component archive"
        );
    }

    Ok(())
}

/// Splits the combined archive into one uncompressed tar per component, each
/// in a temporary file, in the order of the `components` metadata file.
///
/// Uses two streamed passes through the archive.
/// First it reads metadata (`components`, `rust-installer-version`),
/// then it routes entries into per-component archives.
/// This is robust even if the metadata appears late in the archive.
fn split_components(
    archive_bytes: &[u8],
    name: &str,
    version: &str,
    target: &str,
) -> Result<Vec<(String, File)>, String> {
    let prefix = format!("{name}-{version}-{target}/");
    let meta_components_path = format!("{prefix}components");
    let meta_installer_path = format!("{prefix}rust-installer-version");

    let mut component_names: Vec<String> = Vec::new();
    let mut installer_version = String::new();

    // Pass 1: read metadata only.
    let xz_decoder = xz2::read::XzDecoder::new(archive_bytes);
    let mut archive = tar::Archive::new(xz_decoder);
    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read tar entries: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read tar entry: {e}"))?;
        let entry_path = entry
            .path()
            .map_err(|e| format!("Failed to read entry path: {e}"))?
            .to_string_lossy()
            .to_string();

        if entry_path == meta_components_path {
            let mut contents = String::new();
            entry
                .read_to_string(&mut contents)
                .map_err(|e| format!("Failed to read components file: {e}"))?;
            component_names = contents
                .lines()
                .filter(|l| !l.is_empty())
                .map(ToString::to_string)
                .collect();
        } else if entry_path == meta_installer_path {
            entry
                .read_to_string(&mut installer_version)
                .map_err(|e| format!("Failed to read installer version: {e}"))?;
        }

        if !component_names.is_empty() && !installer_version.is_empty() {
            break;
        }
    }

    if component_names.is_empty() {
        return Err("No components file found in archive".to_string());
    }

    // Per-component tar builders created from metadata
    let mut builders: HashMap<String, tar::Builder<BufWriter<File>>> = HashMap::new();
    for cn in &component_names {
        let file =
            tempfile::tempfile().map_err(|e| format!("Failed to create temporary file: {e}"))?;
        let mut builder = tar::Builder::new(BufWriter::new(file));
        builder.mode(tar::HeaderMode::Deterministic);

        if !installer_version.is_empty() {
            append_metadata_file(
                &mut builder,
                &format!("{cn}-{version}-{target}/rust-installer-version"),
                installer_version.as_bytes(),
            )?;
        }
        append_metadata_file(
            &mut builder,
            &format!("{cn}-{version}-{target}/components"),
            format!("{cn}\n").as_bytes(),
        )?;
        builders.insert(cn.clone(), builder);
    }

    // Pass 2: route component entries into their respective builders.
    let xz_decoder = xz2::read::XzDecoder::new(archive_bytes);
    let mut archive = tar::Archive::new(xz_decoder);
    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read tar entries: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read tar entry: {e}"))?;
        let entry_path = entry
            .path()
            .map_err(|e| format!("Failed to read entry path: {e}"))?
            .to_string_lossy()
            .to_string();

        // Skip root metadata files in this pass.
        if entry_path == meta_components_path || entry_path == meta_installer_path {
            continue;
        }

        // Route entry to the matching component builder
        if !entry_path.starts_with(&prefix) {
            continue;
        }
        let rel_path = &entry_path[prefix.len()..];
        // The first path segment is the component name
        let component_name = rel_path.split('/').next().unwrap_or("");
        let Some(builder) = builders.get_mut(component_name) else {
            continue;
        };

        let new_path = format!("{component_name}-{version}-{target}/{rel_path}");

        let mut header = tar::Header::new_gnu();
        header.set_size(entry.size());
        header.set_mode(entry.header().mode().unwrap_or(0o644));
        header.set_entry_type(entry.header().entry_type());
        if let Ok(mtime) = entry.header().mtime() {
            header.set_mtime(mtime);
        }
        if let Ok(Some(link)) = entry.header().link_name() {
            header
                .set_link_name(link)
                .map_err(|e| format!("Failed to set link name: {e}"))?;
        }
        header.set_cksum();

        builder
            .append_data(&mut header, &new_path, &mut entry)
            .map_err(|e| format!("Failed to append to tar: {e}"))?;
    }

    component_names
        .into_iter()
        .map(|component_name| {
            let builder = builders
                .remove(&component_name)
                .ok_or_else(|| format!("Missing builder for {component_name}"))?;
            let file = builder
                .into_inner()
                .and_then(|w| w.into_inner().map_err(std::io::IntoInnerError::into_error))
                .and_then(|mut f| f.rewind().map(|()| f))
                .map_err(|e| format!("Failed to finish tar for {component_name}: {e}"))?;
            Ok((component_name, file))
        })
        .collect()
}

fn compress_component(mut component_tar: File) -> std::io::Result<Vec<u8>> {
    let mut xz_encoder = xz2::write::XzEncoder::new(Vec::new(), 6);
    std::io::copy(&mut component_tar, &mut xz_encoder)?;
    xz_encoder.finish()
}

/// Append a small metadata file (rust-installer-version, components) to a tar builder.
fn append_metadata_file<W: Write>(
    builder: &mut tar::Builder<W>,
    path: &str,
    content: &[u8],
) -> Result<(), String> {
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    builder
        .append_data(&mut header, path, content)
        .map_err(|e| format!("Failed to add {path}: {e}"))
}

fn generate_manifest(
    toolchain: &ToolchainWithTargets,
    settings: &Arc<kellnr_settings::Settings>,
) -> String {
    let base_url = format!("{}/api/v1/toolchains/dist", settings.origin.base_url());

    let mut manifest = String::new();
    let _ = write!(
        manifest,
        r#"manifest-version = "2"
date = "{}"
"#,
        toolchain.date
    );

    // [pkg.rust], the meta-package for the combined archive
    let _ = write!(
        manifest,
        r#"
[pkg.rust]
version = "{}"
"#,
        toolchain.version
    );

    // Only include targets whose component extraction has completed
    let ready_targets: Vec<_> = toolchain
        .targets
        .iter()
        .filter(|t| t.status == ToolchainTargetStatus::Ready)
        .collect();

    for target_info in &ready_targets {
        let archive_url = format!("{}/{}", base_url, target_info.storage_path);
        let _ = write!(
            manifest,
            r#"
[pkg.rust.target.{}]
available = true
xz_url = "{}"
xz_hash = "{}"
"#,
            target_info.target, archive_url, target_info.hash
        );

        // List each component so rustup knows what to install
        for component in &target_info.components {
            let _ = write!(
                manifest,
                r#"
[[pkg.rust.target.{}.components]]
pkg = "{}"
target = "{}"
"#,
                target_info.target, component.name, target_info.target
            );
        }
    }

    // Individual component packages, rustup downloads these one by one
    // Collect unique component names across all targets
    let mut component_pkgs: std::collections::BTreeMap<
        String,
        Vec<(&str, &kellnr_db::ToolchainComponentInfo)>,
    > = std::collections::BTreeMap::new();
    for target_info in &ready_targets {
        for component in &target_info.components {
            component_pkgs
                .entry(component.name.clone())
                .or_default()
                .push((&target_info.target, component));
        }
    }

    for (pkg_name, targets) in &component_pkgs {
        let _ = write!(
            manifest,
            r#"
[pkg.{}]
version = "{}"
"#,
            pkg_name, toolchain.version
        );

        for (target, comp) in targets {
            let comp_url = format!("{}/{}", base_url, comp.storage_path);
            let _ = write!(
                manifest,
                r#"
[pkg.{}.target.{}]
available = true
xz_url = "{}"
xz_hash = "{}"
"#,
                pkg_name, target, comp_url, comp.hash
            );
        }
    }

    manifest
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use axum::response::Response;
    use axum::routing::{delete, get, put};
    use bytes::Bytes;
    use kellnr_common::account_name::UserName;
    use kellnr_common::plain_token::PlainToken;
    use kellnr_db::mock::MockDb;
    use kellnr_db::test_utils::test_drop_table;
    use kellnr_db::{DbProvider, ToolchainTargetInfo, ToolchainWithTargets, UserFlags};
    use kellnr_http::error::ErrorDetails;
    use kellnr_http::state::AppStateData;
    use kellnr_http::test_support::{
        encode_cookies, session_cookie, sqlite_test_state, test_state,
    };
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::toolchain_storage::ToolchainStorage;
    use kellnr_test_db::TestDb;
    use mockall::predicate::*;
    use serde::de::DeserializeOwned;
    use tempfile::TempDir;
    use tower::ServiceExt;

    use super::*;

    const TARGET: &str = "x86_64-unknown-linux-gnu";
    const ARCHIVE_PATH: &str = "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz";
    const UPLOAD_URL: &str = "/api/v1/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15";

    /// App state on a fresh database with `toolchain_storage`, and a session
    /// cookie of the admin.
    async fn admin_state(
        toolchain_storage: Option<Arc<ToolchainStorage>>,
    ) -> (AppStateData, TestDb, String) {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        let state = AppStateData {
            toolchain_storage,
            ..state
        };
        (state, db, cookie)
    }

    /// Create a router with API routes for testing
    fn create_test_router(state: AppStateData) -> Router {
        let api_routes = Router::new()
            .route("/", get(list_toolchains))
            .route("/", put(upload_toolchain))
            .route("/{name}/{version}", delete(delete_toolchain))
            .route(
                "/{name}/{version}/targets/{target}",
                delete(delete_toolchain_target),
            )
            .route("/channels", get(list_channels))
            .route("/channels/{channel}", put(set_channel));

        let dist_routes = Router::new()
            .route("/{manifest_file}", get(get_channel_manifest))
            .route("/{date}/{filename}", get(download_archive));

        Router::new()
            .nest("/api/v1/toolchains", api_routes)
            .nest("/api/v1/toolchains/dist", dist_routes)
            .with_state(state)
    }

    async fn send(state: AppStateData, request: Request<Body>) -> Response<Body> {
        create_test_router(state).oneshot(request).await.unwrap()
    }

    async fn get_with(state: AppStateData, uri: &str, cookie: &str) -> Response<Body> {
        let mut request = Request::get(uri);
        if !cookie.is_empty() {
            request = request.header(header::COOKIE, cookie);
        }
        send(state, request.body(Body::empty()).unwrap()).await
    }

    async fn parse_response<T: DeserializeOwned>(response: Response<Body>) -> T {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn error_detail(response: Response<Body>) -> String {
        let details: ErrorDetails = parse_response(response).await;
        details.errors[0].detail.clone()
    }

    async fn text(response: Response<Body>) -> String {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    fn sample_archive() -> Bytes {
        // Create a minimal valid-looking archive (XZ magic bytes)
        Bytes::from(vec![0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00])
    }

    fn upload_request(uri: &str, cookie: &str) -> Request<Body> {
        Request::put(uri)
            .header(header::COOKIE, cookie)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(sample_archive()))
            .unwrap()
    }

    fn delete_request(uri: &str, cookie: &str) -> Request<Body> {
        Request::delete(uri)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap()
    }

    fn set_channel_request(channel: &str, version: &str, cookie: &str) -> Request<Body> {
        Request::put(format!("/api/v1/toolchains/channels/{channel}"))
            .header(header::COOKIE, cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(format!(
                r#"{{"name": "rust", "version": "{version}"}}"#
            )))
            .unwrap()
    }

    /// Adds the toolchain `rust` 1.0.0 of 2024-01-15 on the channel `stable`,
    /// with a ready target for each of `targets`. Returns the target ids.
    async fn add_toolchain(db: &TestDb, targets: &[&str]) -> Vec<i64> {
        let id = db
            .add_toolchain("rust", "1.0.0", "2024-01-15", Some("stable".to_string()))
            .await
            .unwrap();
        let mut ids = Vec::new();
        for target in targets {
            let target_id = db
                .add_toolchain_target(
                    id,
                    target,
                    &format!("2024-01-15/rust-1.0.0-{target}.tar.xz"),
                    "abc123",
                    1024,
                )
                .await
                .unwrap();
            db.set_target_status(target_id, ToolchainTargetStatus::Ready)
                .await
                .unwrap();
            ids.push(target_id);
        }
        ids
    }

    /// The toolchain `rust` 1.0.0, if it exists.
    async fn toolchain(db: &TestDb) -> Option<ToolchainWithTargets> {
        db.get_toolchain_by_version("rust", "1.0.0").await.unwrap()
    }

    fn target_names(toolchain: &ToolchainWithTargets) -> Vec<&str> {
        toolchain
            .targets
            .iter()
            .map(|t| t.target.as_str())
            .collect()
    }

    fn stored(path: &str) -> StorageKey {
        StorageKey::from_stored(path.to_string())
    }

    fn fs_toolchain_storage(dir: &TempDir) -> Arc<ToolchainStorage> {
        let storage: DynStorage = Box::new(FSStorage::new(dir.path().to_str().unwrap()).unwrap());
        Arc::new(ToolchainStorage::new(storage))
    }

    // ==================== Authorization Tests ====================

    #[tokio::test]
    async fn admin_endpoints_forbid_non_admin() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        let requests = [
            Request::get("/api/v1/toolchains")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
            upload_request(UPLOAD_URL, &cookie),
            delete_request("/api/v1/toolchains/rust/1.0.0", &cookie),
            delete_request(
                "/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnu",
                &cookie,
            ),
            Request::get("/api/v1/toolchains/channels")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
            set_channel_request("stable", "1.0.0", &cookie),
        ];

        for request in requests {
            let uri = request.uri().to_string();
            let response = send(state.clone(), request).await;
            assert_eq!(StatusCode::FORBIDDEN, response.status(), "{uri}");
        }
    }

    // ==================== Happy Path Tests ====================

    #[tokio::test]
    async fn test_list_toolchains_returns_data() {
        let (state, db, cookie) = admin_state(None).await;
        add_toolchain(&db, &[TARGET]).await;

        let response = get_with(state, "/api/v1/toolchains", &cookie).await;

        assert_eq!(StatusCode::OK, response.status());

        let toolchains: Vec<ToolchainWithTargets> = parse_response(response).await;
        assert_eq!(toolchains.len(), 1);
        assert_eq!(toolchains[0].name, "rust");
        assert_eq!(toolchains[0].version, "1.0.0");
        assert_eq!(toolchains[0].targets.len(), 1);
    }

    #[tokio::test]
    async fn test_list_channels_returns_data() {
        let (state, db, cookie) = admin_state(None).await;
        add_toolchain(&db, &[]).await;

        let response = get_with(state, "/api/v1/toolchains/channels", &cookie).await;

        assert_eq!(StatusCode::OK, response.status());

        let channels: Vec<ChannelInfo> = parse_response(response).await;
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "stable");
        assert_eq!(channels[0].version, "1.0.0");
    }

    #[tokio::test]
    async fn test_upload_toolchain_success() {
        let temp_dir = TempDir::new().unwrap();
        let (state, db, cookie) = admin_state(Some(fs_toolchain_storage(&temp_dir))).await;

        let response = send(
            state,
            upload_request(&format!("{UPLOAD_URL}&channel=stable"), &cookie),
        )
        .await;

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
        assert!(result.message.unwrap().contains("Uploaded"));
        let toolchain = toolchain(&db).await.unwrap();
        assert_eq!(toolchain.date, "2024-01-15");
        assert_eq!(toolchain.channel.as_deref(), Some("stable"));
        assert_eq!(target_names(&toolchain), vec![TARGET]);
        assert_eq!(toolchain.targets[0].storage_path, ARCHIVE_PATH);
    }

    #[tokio::test]
    async fn test_set_channel_success() {
        let (state, db, cookie) = admin_state(None).await;
        add_toolchain(&db, &[]).await;

        let response = send(state, set_channel_request("beta", "1.0.0", &cookie)).await;

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
        assert_eq!(
            toolchain(&db).await.unwrap().channel.as_deref(),
            Some("beta")
        );
    }

    #[tokio::test]
    async fn test_delete_toolchain_target_success() {
        let temp_dir = TempDir::new().unwrap();
        let (state, db, cookie) = admin_state(Some(fs_toolchain_storage(&temp_dir))).await;
        // Two targets so deleting one doesn't trigger auto-delete of toolchain
        add_toolchain(&db, &[TARGET, "aarch64-unknown-linux-gnu"]).await;

        let response = send(
            state,
            delete_request(
                "/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnu",
                &cookie,
            ),
        )
        .await;

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
        let toolchain = toolchain(&db).await.unwrap();
        assert_eq!(target_names(&toolchain), vec!["aarch64-unknown-linux-gnu"]);
    }

    #[tokio::test]
    async fn test_delete_last_target_auto_deletes_toolchain() {
        let temp_dir = TempDir::new().unwrap();
        let (state, db, cookie) = admin_state(Some(fs_toolchain_storage(&temp_dir))).await;
        add_toolchain(&db, &[TARGET]).await;

        let response = send(
            state,
            delete_request(
                "/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnu",
                &cookie,
            ),
        )
        .await;

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
        // Message should indicate the whole toolchain was deleted (last target removed)
        let msg = result.message.unwrap();
        assert!(
            msg.contains("last target removed"),
            "Expected 'last target removed' in message: {msg}"
        );
        assert!(toolchain(&db).await.is_none());
    }

    #[tokio::test]
    async fn test_delete_toolchain_success() {
        let temp_dir = TempDir::new().unwrap();
        let (state, db, cookie) = admin_state(Some(fs_toolchain_storage(&temp_dir))).await;
        add_toolchain(&db, &[TARGET, "aarch64-unknown-linux-gnu"]).await;

        let response = send(
            state,
            delete_request("/api/v1/toolchains/rust/1.0.0", &cookie),
        )
        .await;

        assert_eq!(StatusCode::OK, response.status());

        let result: ToolchainResponse = parse_response(response).await;
        assert!(result.success);
        assert!(result.message.unwrap().contains("Deleted"));
        assert!(toolchain(&db).await.is_none());
    }

    #[tokio::test]
    async fn test_delete_toolchain_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let (state, _db, cookie) = admin_state(Some(fs_toolchain_storage(&temp_dir))).await;

        let response = send(
            state,
            delete_request("/api/v1/toolchains/rust/99.99.99", &cookie),
        )
        .await;

        assert_eq!(StatusCode::NOT_FOUND, response.status());

        assert!(error_detail(response).await.contains("not found"));
    }

    // ==================== Error Path Tests ====================

    #[tokio::test]
    async fn test_upload_toolchain_storage_not_configured() {
        // No toolchain storage configured
        let (state, _db, cookie) = admin_state(None).await;

        let response = send(state, upload_request(UPLOAD_URL, &cookie)).await;

        assert_eq!(StatusCode::SERVICE_UNAVAILABLE, response.status());

        assert!(error_detail(response).await.contains("not configured"));
    }

    #[tokio::test]
    async fn test_upload_duplicate_target_conflict() {
        let temp_dir = TempDir::new().unwrap();
        let (state, db, cookie) = admin_state(Some(fs_toolchain_storage(&temp_dir))).await;
        add_toolchain(&db, &[TARGET]).await;

        let response = send(state, upload_request(UPLOAD_URL, &cookie)).await;

        assert_eq!(StatusCode::CONFLICT, response.status());

        assert!(error_detail(response).await.contains("already exists"));
    }

    #[tokio::test]
    async fn test_set_channel_toolchain_not_found() {
        let (state, _db, cookie) = admin_state(None).await;

        let response = send(state, set_channel_request("stable", "99.99.99", &cookie)).await;

        assert_eq!(StatusCode::NOT_FOUND, response.status());

        assert!(error_detail(response).await.contains("not found"));
    }

    #[tokio::test]
    async fn test_get_manifest_invalid_format_not_found() {
        // Missing prefix
        let response = get_with(test_state(), "/api/v1/toolchains/dist/stable.toml", "").await;

        assert_eq!(StatusCode::NOT_FOUND, response.status());
    }

    #[tokio::test]
    async fn test_get_manifest_channel_not_found() {
        let (state, _db) = sqlite_test_state().await;

        let response = get_with(
            state,
            "/api/v1/toolchains/dist/channel-rust-nonexistent.toml",
            "",
        )
        .await;

        assert_eq!(StatusCode::NOT_FOUND, response.status());
    }

    #[tokio::test]
    async fn test_download_archive_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let state = AppStateData {
            toolchain_storage: Some(fs_toolchain_storage(&temp_dir)),
            ..test_state()
        };

        let response = get_with(
            state,
            "/api/v1/toolchains/dist/2024-01-15/nonexistent.tar.xz",
            "",
        )
        .await;

        assert_eq!(StatusCode::NOT_FOUND, response.status());
    }

    #[tokio::test]
    async fn test_download_archive_storage_not_configured() {
        // No toolchain storage
        let response = get_with(
            test_state(),
            "/api/v1/toolchains/dist/2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz",
            "",
        )
        .await;

        assert_eq!(StatusCode::SERVICE_UNAVAILABLE, response.status());
    }

    // ==================== Manifest Generation Test ====================

    #[tokio::test]
    async fn test_get_manifest_returns_valid_toml() {
        let (state, db) = sqlite_test_state().await;
        add_toolchain(&db, &[TARGET]).await;

        let response = get_with(
            state,
            "/api/v1/toolchains/dist/channel-rust-stable.toml",
            "",
        )
        .await;

        assert_eq!(StatusCode::OK, response.status());
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/toml; charset=utf-8"
        );

        let manifest = text(response).await;

        // Verify manifest structure
        assert!(manifest.contains("manifest-version = \"2\""));
        assert!(manifest.contains("date = \"2024-01-15\""));
        assert!(manifest.contains("[pkg.rust]"));
        assert!(manifest.contains("version = \"1.0.0\""));
        assert!(manifest.contains("[pkg.rust.target.x86_64-unknown-linux-gnu]"));
        assert!(manifest.contains("available = true"));
        assert!(manifest.contains("xz_url = \""));
        assert!(manifest.contains("xz_hash = \"abc123\""));
    }

    #[tokio::test]
    async fn test_get_manifest_by_version_falls_back_when_no_channel() {
        let (state, db) = sqlite_test_state().await;
        add_toolchain(&db, &[TARGET]).await;

        // "1.0.0" is not a channel name, so the version lookup finds the toolchain
        let response = get_with(state, "/api/v1/toolchains/dist/channel-rust-1.0.0.toml", "").await;

        assert_eq!(StatusCode::OK, response.status());

        let manifest = text(response).await;

        assert!(manifest.contains("manifest-version = \"2\""));
        assert!(manifest.contains("version = \"1.0.0\""));
        assert!(manifest.contains("[pkg.rust.target.x86_64-unknown-linux-gnu]"));
    }

    #[tokio::test]
    async fn test_manifest_includes_component_packages() {
        let (state, db) = sqlite_test_state().await;
        let target_id = add_toolchain(&db, &[TARGET]).await[0];
        for (name, hash, size) in [("rustc", "rustc_hash", 512), ("cargo", "cargo_hash", 256)] {
            db.add_toolchain_component(
                target_id,
                name,
                &format!("2024-01-15/{name}-1.0.0-x86_64-unknown-linux-gnu.tar.xz"),
                hash,
                size,
            )
            .await
            .unwrap();
        }

        let response = get_with(
            state,
            "/api/v1/toolchains/dist/channel-rust-stable.toml",
            "",
        )
        .await;

        assert_eq!(StatusCode::OK, response.status());

        let manifest = text(response).await;

        // Component list under the rust meta-package
        assert!(manifest.contains("[[pkg.rust.target.x86_64-unknown-linux-gnu.components]]"));
        assert!(manifest.contains("pkg = \"rustc\""));
        assert!(manifest.contains("pkg = \"cargo\""));

        // Individual component packages with download URLs
        assert!(manifest.contains("[pkg.rustc]"));
        assert!(manifest.contains("[pkg.rustc.target.x86_64-unknown-linux-gnu]"));
        assert!(manifest.contains("xz_hash = \"rustc_hash\""));

        assert!(manifest.contains("[pkg.cargo]"));
        assert!(manifest.contains("[pkg.cargo.target.x86_64-unknown-linux-gnu]"));
        assert!(manifest.contains("xz_hash = \"cargo_hash\""));
    }

    /// A combined archive of the toolchain `rust` 1.0.0 for
    /// `x86_64-unknown-linux-gnu` with the components `rustc` and `cargo`.
    fn two_component_archive() -> Vec<u8> {
        use std::io::Write;

        // Build a minimal combined archive with two components
        let mut tar_builder = tar::Builder::new(Vec::new());

        // rust-installer-version
        let version_data = b"3\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(version_data.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        tar_builder
            .append_data(
                &mut header,
                "rust-1.0.0-x86_64-unknown-linux-gnu/rust-installer-version",
                &version_data[..],
            )
            .unwrap();

        // components file
        let components_data = b"rustc\ncargo\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(components_data.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        tar_builder
            .append_data(
                &mut header,
                "rust-1.0.0-x86_64-unknown-linux-gnu/components",
                &components_data[..],
            )
            .unwrap();

        // rustc/manifest.in
        let manifest_data = b"file:bin/rustc\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest_data.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        tar_builder
            .append_data(
                &mut header,
                "rust-1.0.0-x86_64-unknown-linux-gnu/rustc/manifest.in",
                &manifest_data[..],
            )
            .unwrap();

        // cargo/manifest.in
        let manifest_data = b"file:bin/cargo\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest_data.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        tar_builder
            .append_data(
                &mut header,
                "rust-1.0.0-x86_64-unknown-linux-gnu/cargo/manifest.in",
                &manifest_data[..],
            )
            .unwrap();

        let tar_data = tar_builder.into_inner().unwrap();

        // Compress with xz
        let mut xz_encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
        xz_encoder.write_all(&tar_data).unwrap();
        xz_encoder.finish().unwrap()
    }

    const ARCHIVE_ID: ToolchainArchiveId<'static> = ToolchainArchiveId {
        name: "rust",
        version: "1.0.0",
        date: "2024-01-15",
        target: TARGET,
    };

    #[tokio::test]
    async fn test_extract_components_from_archive() {
        let xz_data = two_component_archive();

        // Set up storage and database
        let tmp_dir = TempDir::new().unwrap();
        let toolchain_dir = tmp_dir.path().join("toolchains");
        std::fs::create_dir_all(&toolchain_dir).unwrap();
        let fs = Box::new(FSStorage::new(toolchain_dir.to_str().unwrap()).unwrap()) as DynStorage;
        let storage = Arc::new(ToolchainStorage::new(fs));
        let db = TestDb::sqlite().await;
        let target_id = add_toolchain(&db, &[TARGET]).await[0];

        let result = extract_and_store_components(
            &storage,
            &db.provider(),
            target_id,
            &ARCHIVE_ID,
            Bytes::from(xz_data),
        )
        .await;

        assert!(result.is_ok(), "extraction failed: {result:?}");

        let mut components: Vec<String> = toolchain(&db).await.unwrap().targets[0]
            .components
            .iter()
            .map(|c| c.name.clone())
            .collect();
        components.sort_unstable();
        assert_eq!(components, vec!["cargo", "rustc"]);

        // Verify component archives were stored
        let rustc_exists = storage
            .exists(&stored(
                "2024-01-15/rustc-1.0.0-x86_64-unknown-linux-gnu.tar.xz",
            ))
            .await
            .unwrap();
        assert!(rustc_exists, "rustc component archive not found in storage");

        let cargo_exists = storage
            .exists(&stored(
                "2024-01-15/cargo-1.0.0-x86_64-unknown-linux-gnu.tar.xz",
            ))
            .await
            .unwrap();
        assert!(cargo_exists, "cargo component archive not found in storage");

        // Verify the component archive contains rust-installer-version and components files
        let rustc_archive = storage
            .get(&stored(
                "2024-01-15/rustc-1.0.0-x86_64-unknown-linux-gnu.tar.xz",
            ))
            .await
            .unwrap();
        let mut xz_dec = xz2::read::XzDecoder::new(&rustc_archive[..]);
        let mut tar_bytes = Vec::new();
        xz_dec.read_to_end(&mut tar_bytes).unwrap();
        let mut archive = tar::Archive::new(Cursor::new(&tar_bytes));
        let mut entry_names: Vec<String> = Vec::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().to_string_lossy().to_string();
            if path.ends_with("manifest.in") {
                let mut contents = String::new();
                entry.read_to_string(&mut contents).unwrap();
                assert_eq!("file:bin/rustc\n", contents);
            }
            entry_names.push(path);
        }
        assert!(
            entry_names
                .iter()
                .any(|n| n.contains("rust-installer-version")),
            "missing rust-installer-version in component archive: {entry_names:?}"
        );
        assert!(
            entry_names.iter().any(|n| n.contains("components")),
            "missing components file in component archive: {entry_names:?}"
        );
        assert!(
            entry_names.iter().any(|n| n.contains("manifest.in")),
            "missing manifest.in in component archive: {entry_names:?}"
        );
    }

    const RUSTC_COMPONENT: &str = "2024-01-15/rustc-1.0.0-x86_64-unknown-linux-gnu.tar.xz";

    /// Extracts [`two_component_archive`] into `storage` after a leftover file
    /// was stored at the path of the rustc component, which another target
    /// refers to if `in_use`.
    async fn extract_over_leftover(
        storage: &Arc<ToolchainStorage>,
        in_use: bool,
    ) -> Result<(), String> {
        storage
            .put_raw(&stored(RUSTC_COMPONENT), Bytes::from_static(b"leftover"))
            .await
            .unwrap();
        let db = TestDb::sqlite().await;
        let ids = add_toolchain(&db, &[TARGET, "aarch64-unknown-linux-gnu"]).await;
        if in_use {
            db.add_toolchain_component(ids[1], "rustc", RUSTC_COMPONENT, "hash", 8)
                .await
                .unwrap();
        }

        extract_and_store_components(
            storage,
            &db.provider(),
            ids[0],
            &ARCHIVE_ID,
            Bytes::from(two_component_archive()),
        )
        .await
    }

    #[tokio::test]
    async fn extract_components_replaces_leftover_component_archive() {
        let dir = TempDir::new().unwrap();
        let storage = fs_toolchain_storage(&dir);

        let result = extract_over_leftover(&storage, false).await;

        assert!(result.is_ok(), "extraction failed: {result:?}");
        let stored = storage.get(&stored(RUSTC_COMPONENT)).await.unwrap();
        assert_ne!(Bytes::from_static(b"leftover"), stored);
    }

    #[tokio::test]
    async fn extract_components_keeps_component_archive_of_other_target() {
        let dir = TempDir::new().unwrap();
        let storage = fs_toolchain_storage(&dir);

        let result = extract_over_leftover(&storage, true).await;

        assert!(result.is_err());
        let stored = storage.get(&stored(RUSTC_COMPONENT)).await.unwrap();
        assert_eq!(Bytes::from_static(b"leftover"), stored);
    }

    #[tokio::test]
    async fn test_get_manifest_sha256_returns_ok() {
        let (state, db) = sqlite_test_state().await;
        add_toolchain(&db, &[TARGET]).await;

        // rustup requests the .sha256 of the manifest before downloading
        // the manifest itself. This must return a valid SHA256 hash.
        let response = get_with(
            state,
            "/api/v1/toolchains/dist/channel-rust-stable.toml.sha256",
            "",
        )
        .await;

        assert_eq!(StatusCode::OK, response.status());

        let hash_content = text(response).await;

        // Should be a valid hex-encoded SHA256 hash (64 characters)
        let hash = hash_content.trim();
        assert_eq!(
            hash.len(),
            64,
            "SHA256 hash should be 64 hex characters, got: {hash}"
        );
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "SHA256 hash should only contain hex characters"
        );
    }

    fn ready_toolchain(targets: &[&str]) -> ToolchainWithTargets {
        ToolchainWithTargets {
            id: 1,
            name: "rust".to_string(),
            version: "1.0.0".to_string(),
            date: "2024-01-15".to_string(),
            channel: Some("stable".to_string()),
            created: "2024-01-15".to_string(),
            targets: targets
                .iter()
                .zip(1..)
                .map(|(target, id)| ToolchainTargetInfo {
                    id,
                    target: (*target).to_string(),
                    storage_path: format!("2024-01-15/rust-1.0.0-{target}.tar.xz"),
                    hash: "abc123".to_string(),
                    size: 1024,
                    components: vec![],
                    status: ToolchainTargetStatus::Ready,
                })
                .collect(),
        }
    }

    #[test]
    fn manifest_urls_use_the_origin_path_and_omit_the_default_port() {
        let mut settings = kellnr_settings::test_settings();
        settings.origin.hostname = "kellnr.example.com".to_string();
        settings.origin.protocol = kellnr_settings::Protocol::Https;
        settings.origin.port = 443;
        settings.origin.path = "/registry".to_string();

        let manifest = generate_manifest(
            &ready_toolchain(&["x86_64-unknown-linux-gnu"]),
            &Arc::new(settings),
        );

        assert!(
            manifest.contains(
                "xz_url = \"https://kellnr.example.com/registry/api/v1/toolchains/dist/2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz\""
            ),
            "unexpected manifest: {manifest}"
        );
    }

    /// A misspelled target must not count as the last target, which would
    /// delete the whole toolchain.
    #[tokio::test]
    async fn delete_unknown_target_of_single_target_toolchain_is_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let (state, db, cookie) = admin_state(Some(fs_toolchain_storage(&temp_dir))).await;
        add_toolchain(&db, &[TARGET]).await;

        let response = send(
            state,
            delete_request(
                "/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnux",
                &cookie,
            ),
        )
        .await;

        assert_eq!(StatusCode::NOT_FOUND, response.status());
        assert_eq!(target_names(&toolchain(&db).await.unwrap()), vec![TARGET]);
    }

    #[tokio::test]
    async fn upload_does_not_create_toolchain_when_archive_cannot_be_stored() {
        let temp_dir = TempDir::new().unwrap();
        let storage = fs_toolchain_storage(&temp_dir);
        storage
            .put_raw(&stored(ARCHIVE_PATH), Bytes::from_static(b"orphan"))
            .await
            .unwrap();
        let (state, db, cookie) = admin_state(Some(storage)).await;

        let response = send(state, upload_request(UPLOAD_URL, &cookie)).await;

        assert_eq!(StatusCode::CONFLICT, response.status());
        assert!(toolchain(&db).await.is_none());
    }

    #[tokio::test]
    async fn upload_removes_archive_and_new_toolchain_when_target_cannot_be_added() {
        let temp_dir = TempDir::new().unwrap();
        let storage = fs_toolchain_storage(&temp_dir);
        let (state, db, cookie) = admin_state(Some(storage.clone())).await;
        test_drop_table(&db, "toolchain_target").await.unwrap();

        let response = send(state, upload_request(UPLOAD_URL, &cookie)).await;

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status());
        assert!(!error_detail(response).await.contains("no such table"));
        assert!(!storage.exists(&stored(ARCHIVE_PATH)).await.unwrap());
        assert!(toolchain(&db).await.is_none());
    }

    // The database cannot fail the delete of a toolchain while it still
    // reads it, so these tests keep a mock.

    /// Create an encrypted admin session cookie
    fn admin_cookie() -> String {
        encode_cookies([("kellnr_session_id", "admin_session")])
    }

    /// Database with a session of the admin that holds the toolchain `rust`
    /// 1.0.0 with `targets`.
    fn admin_db(targets: &'static [&'static str]) -> MockDb {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("admin_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("admin").unwrap(),
                    is_admin: true,
                    is_read_only: false,
                })
            });
        mock_db
            .expect_get_toolchain_by_version()
            .returning(|_, _| Ok(Some(ready_toolchain(targets))));
        mock_db
    }

    /// Stands in for a database error whose text must not reach the client.
    fn db_error() -> DbError {
        DbError::InitializationError("secret detail".to_string())
    }

    /// State with `mock_db` and a toolchain storage that holds the archive of
    /// the `x86_64-unknown-linux-gnu` target.
    async fn state_with_archive(
        mock_db: MockDb,
        dir: &TempDir,
    ) -> (AppStateData, Arc<ToolchainStorage>) {
        let storage = fs_toolchain_storage(dir);
        storage
            .put_raw(&stored(ARCHIVE_PATH), Bytes::from_static(b"archive"))
            .await
            .unwrap();
        let state = AppStateData {
            db: Arc::new(mock_db),
            toolchain_storage: Some(storage.clone()),
            ..test_state()
        };
        (state, storage)
    }

    #[tokio::test]
    async fn delete_toolchain_keeps_archives_when_database_delete_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock_db = admin_db(&[TARGET]);
        mock_db
            .expect_delete_toolchain()
            .returning(|_, _| Err(db_error()));
        let (state, storage) = state_with_archive(mock_db, &temp_dir).await;

        let response = send(
            state,
            delete_request("/api/v1/toolchains/rust/1.0.0", &admin_cookie()),
        )
        .await;

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status());
        assert!(!error_detail(response).await.contains("secret detail"));
        assert!(storage.exists(&stored(ARCHIVE_PATH)).await.unwrap());
    }

    #[tokio::test]
    async fn delete_target_keeps_archives_when_database_delete_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock_db = admin_db(&[TARGET, "aarch64-unknown-linux-gnu"]);
        mock_db
            .expect_delete_toolchain_target()
            .returning(|_, _, _| Err(db_error()));
        let (state, storage) = state_with_archive(mock_db, &temp_dir).await;

        let response = send(
            state,
            delete_request(
                "/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnu",
                &admin_cookie(),
            ),
        )
        .await;

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status());
        assert!(storage.exists(&stored(ARCHIVE_PATH)).await.unwrap());
    }

    #[tokio::test]
    async fn database_errors_are_not_returned_to_the_client() {
        let temp_dir = TempDir::new().unwrap();
        let (state, db, cookie) = admin_state(Some(fs_toolchain_storage(&temp_dir))).await;
        test_drop_table(&db, "toolchain").await.unwrap();

        let response = send(
            state,
            delete_request("/api/v1/toolchains/rust/1.0.0", &cookie),
        )
        .await;

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status());
        assert!(!error_detail(response).await.contains("no such table"));
    }

    #[tokio::test]
    async fn target_is_marked_failed_when_processing_panics() {
        let db = TestDb::sqlite().await;
        let id = db
            .add_toolchain("rust", "1.0.0", "2024-01-15", None)
            .await
            .unwrap();
        let target_id = db
            .add_toolchain_target(id, TARGET, ARCHIVE_PATH, "abc123", 1024)
            .await
            .unwrap();

        spawn_target_processing(&BackgroundTasks::new(), db.provider(), target_id, async {
            panic!("extraction bug")
        })
        .await
        .unwrap();

        let status = &toolchain(&db).await.unwrap().targets[0].status;
        assert_eq!(*status, ToolchainTargetStatus::Failed);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_waits_for_target_processing() {
        let tasks = BackgroundTasks::new();
        let finished = Arc::new(AtomicBool::new(false));
        let finished_in_task = finished.clone();

        spawn_target_processing(&tasks, test_state().db, 7, async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            finished_in_task.store(true, Ordering::SeqCst);
        });
        tasks.request_shutdown();
        tasks.wait_for_shutdown(Duration::from_secs(30)).await;

        assert!(finished.load(Ordering::SeqCst));
    }
}
