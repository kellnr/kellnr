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
    use kellnr_db::{
        DbProvider, ToolchainComponentInfo, ToolchainTargetInfo, ToolchainWithTargets,
    };
    use kellnr_http::error::ErrorDetails;
    use kellnr_http::state::AppStateData;
    use kellnr_http::test_support::encode_cookies;
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::toolchain_storage::ToolchainStorage;
    use mockall::predicate::*;
    use serde::de::DeserializeOwned;
    use tempfile::TempDir;
    use tower::ServiceExt;

    use super::*;

    /// Create an encrypted admin session cookie
    fn admin_cookie() -> String {
        encode_cookies([("kellnr_session_id", "admin_session")])
    }

    /// Create an encrypted non-admin session cookie
    fn non_admin_cookie() -> String {
        encode_cookies([("kellnr_session_id", "user_session")])
    }

    /// Create app state with the given database and toolchain storage
    fn create_app_state(
        db: Arc<dyn DbProvider>,
        toolchain_storage: Option<Arc<ToolchainStorage>>,
    ) -> AppStateData {
        AppStateData {
            db,
            toolchain_storage,
            ..kellnr_http::test_support::test_state()
        }
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

    async fn parse_response<T: DeserializeOwned>(response: Response<Body>) -> T {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn error_detail(response: Response<Body>) -> String {
        let details: ErrorDetails = parse_response(response).await;
        details.errors[0].detail.clone()
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
            .with(eq(PlainToken::from("admin_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("admin").unwrap(),
                    is_admin: true,
                    is_read_only: false,
                })
            });
        mock_db.expect_list_toolchains().returning(|| Ok(vec![]));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains")
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
            .with(eq(PlainToken::from("user_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("user").unwrap(),
                    is_admin: false,
                    is_read_only: false,
                })
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains")
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
                Request::get("/api/v1/toolchains")
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
            .with(eq(PlainToken::from("user_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("user").unwrap(),
                    is_admin: false,
                    is_read_only: false,
                })
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::put(
                    "/api/v1/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15",
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
            .with(eq(PlainToken::from("user_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("user").unwrap(),
                    is_admin: false,
                    is_read_only: false,
                })
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::delete("/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnu")
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
            .with(eq(PlainToken::from("user_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("user").unwrap(),
                    is_admin: false,
                    is_read_only: false,
                })
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains/channels")
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
            .with(eq(PlainToken::from("user_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("user").unwrap(),
                    is_admin: false,
                    is_read_only: false,
                })
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let payload = r#"{"name": "rust", "version": "1.0.0"}"#;

        let response = router
            .oneshot(
                Request::put("/api/v1/toolchains/channels/stable")
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
            .with(eq(PlainToken::from("admin_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("admin").unwrap(),
                    is_admin: true,
                    is_read_only: false,
                })
            });
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
                    storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                        .to_string(),
                    hash: "abc123".to_string(),
                    size: 1024,
                    components: vec![],
                    status: ToolchainTargetStatus::Ready,
                }],
            }])
        });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains")
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
            .with(eq(PlainToken::from("admin_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("admin").unwrap(),
                    is_admin: true,
                    is_read_only: false,
                })
            });
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
                Request::get("/api/v1/toolchains/channels")
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
        let storage: DynStorage =
            Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

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
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| Ok(None));
        mock_db
            .expect_add_toolchain()
            .with(
                eq("rust"),
                eq("1.0.0"),
                eq("2024-01-15"),
                eq(Some("stable".to_string())),
            )
            .returning(|_, _, _, _| Ok(1));
        mock_db
            .expect_add_toolchain_target()
            .returning(|_, _, _, _, _| Ok(1));

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::put(
                    "/api/v1/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15&channel=stable",
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
                Request::put("/api/v1/toolchains/channels/stable")
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
        let storage: DynStorage =
            Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

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
                            storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                                .to_string(),
                            hash: "abc123".to_string(),
                            size: 1024,
                            components: vec![],
                            status: ToolchainTargetStatus::Ready,
                        },
                        ToolchainTargetInfo {
                            id: 2,
                            target: "aarch64-unknown-linux-gnu".to_string(),
                            storage_path: "2024-01-15/rust-1.0.0-aarch64-unknown-linux-gnu.tar.xz"
                                .to_string(),
                            hash: "def456".to_string(),
                            size: 2048,
                            components: vec![],
                            status: ToolchainTargetStatus::Ready,
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
                Request::delete("/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnu")
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
        let storage: DynStorage =
            Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

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
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                            .to_string(),
                        hash: "abc123".to_string(),
                        size: 1024,
                        components: vec![],
                        status: ToolchainTargetStatus::Ready,
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
                Request::delete("/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnu")
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
        assert!(
            msg.contains("last target removed"),
            "Expected 'last target removed' in message: {msg}"
        );
    }

    #[tokio::test]
    async fn test_delete_toolchain_success() {
        let temp_dir = TempDir::new().unwrap();
        let storage: DynStorage =
            Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

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
                            storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                                .to_string(),
                            hash: "abc123".to_string(),
                            size: 1024,
                            components: vec![],
                            status: ToolchainTargetStatus::Ready,
                        },
                        ToolchainTargetInfo {
                            id: 2,
                            target: "aarch64-unknown-linux-gnu".to_string(),
                            storage_path: "2024-01-15/rust-1.0.0-aarch64-unknown-linux-gnu.tar.xz"
                                .to_string(),
                            hash: "def456".to_string(),
                            size: 2048,
                            components: vec![],
                            status: ToolchainTargetStatus::Ready,
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
                Request::delete("/api/v1/toolchains/rust/1.0.0")
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
        let storage: DynStorage =
            Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

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
            .with(eq("rust"), eq("99.99.99"))
            .returning(|_, _| Ok(None));

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::delete("/api/v1/toolchains/rust/99.99.99")
                    .header(header::COOKIE, admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, response.status());

        assert!(error_detail(response).await.contains("not found"));
    }

    #[tokio::test]
    async fn test_delete_entire_toolchain_non_admin_forbidden() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .with(eq(PlainToken::from("user_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("user").unwrap(),
                    is_admin: false,
                    is_read_only: false,
                })
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::delete("/api/v1/toolchains/rust/1.0.0")
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
            .with(eq(PlainToken::from("admin_session")))
            .returning(|_| {
                Ok(kellnr_db::SessionInfo {
                    name: UserName::new("admin").unwrap(),
                    is_admin: true,
                    is_read_only: false,
                })
            });

        // No toolchain storage configured
        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::put(
                    "/api/v1/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15",
                )
                .header(header::COOKIE, admin_cookie())
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(sample_archive()))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::SERVICE_UNAVAILABLE, response.status());

        assert!(error_detail(response).await.contains("not configured"));
    }

    #[tokio::test]
    async fn test_upload_duplicate_target_conflict() {
        let temp_dir = TempDir::new().unwrap();
        let storage: DynStorage =
            Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

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
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                            .to_string(),
                        hash: "abc123".to_string(),
                        size: 1024,
                        components: vec![],
                        status: ToolchainTargetStatus::Ready,
                    }],
                }))
            });

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::put(
                    "/api/v1/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15",
                )
                .header(header::COOKIE, admin_cookie())
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(sample_archive()))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::CONFLICT, response.status());

        assert!(error_detail(response).await.contains("already exists"));
    }

    #[tokio::test]
    async fn test_set_channel_toolchain_not_found() {
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
            .expect_set_channel()
            .with(eq("stable"), eq("rust"), eq("99.99.99"))
            .returning(|_, name, version| {
                Err(DbError::ToolchainNotFound(
                    name.to_string(),
                    version.to_string(),
                ))
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let payload = r#"{"name": "rust", "version": "99.99.99"}"#;

        let response = router
            .oneshot(
                Request::put("/api/v1/toolchains/channels/stable")
                    .header(header::COOKIE, admin_cookie())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, response.status());

        assert!(error_detail(response).await.contains("not found"));
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
                Request::get("/api/v1/toolchains/dist/stable.toml")
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
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("nonexistent"))
            .returning(|_, _| Ok(None));

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains/dist/channel-rust-nonexistent.toml")
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
        let storage: DynStorage =
            Box::new(FSStorage::new(temp_dir.path().to_str().unwrap()).unwrap());
        let toolchain_storage = Some(Arc::new(ToolchainStorage::new(storage)));

        let mock_db = MockDb::new();

        let state = create_app_state(Arc::new(mock_db), toolchain_storage);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains/dist/2024-01-15/nonexistent.tar.xz")
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
                Request::get(
                    "/api/v1/toolchains/dist/2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz",
                )
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
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                            .to_string(),
                        hash: "abc123def456".to_string(),
                        size: 1024,
                        components: vec![],
                        status: ToolchainTargetStatus::Ready,
                    }],
                }))
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains/dist/channel-rust-stable.toml")
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
        assert!(manifest.contains("xz_url = \""));
        assert!(manifest.contains("xz_hash = \"abc123def456\""));
    }

    #[tokio::test]
    async fn test_get_manifest_by_version_falls_back_when_no_channel() {
        let mut mock_db = MockDb::new();
        // Channel lookup returns None, version "1.0.0" is not a channel name
        mock_db
            .expect_get_toolchain_by_channel()
            .with(eq("1.0.0"))
            .returning(|_| Ok(None));
        // Fallback: version lookup succeeds
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
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                            .to_string(),
                        hash: "abc123def456".to_string(),
                        size: 1024,
                        components: vec![],
                        status: ToolchainTargetStatus::Ready,
                    }],
                }))
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains/dist/channel-rust-1.0.0.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let manifest = String::from_utf8(body.to_vec()).unwrap();

        assert!(manifest.contains("manifest-version = \"2\""));
        assert!(manifest.contains("version = \"1.0.0\""));
        assert!(manifest.contains("[pkg.rust.target.x86_64-unknown-linux-gnu]"));
    }

    #[tokio::test]
    async fn test_manifest_includes_component_packages() {
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
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                            .to_string(),
                        hash: "abc123".to_string(),
                        size: 1024,
                        components: vec![
                            ToolchainComponentInfo {
                                name: "rustc".to_string(),
                                storage_path:
                                    "2024-01-15/rustc-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                                        .to_string(),
                                hash: "rustc_hash".to_string(),
                                size: 512,
                            },
                            ToolchainComponentInfo {
                                name: "cargo".to_string(),
                                storage_path:
                                    "2024-01-15/cargo-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                                        .to_string(),
                                hash: "cargo_hash".to_string(),
                                size: 256,
                            },
                        ],
                        status: ToolchainTargetStatus::Ready,
                    }],
                }))
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains/dist/channel-rust-stable.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let manifest = String::from_utf8(body.to_vec()).unwrap();

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

    #[tokio::test]
    async fn test_extract_components_from_archive() {
        use tempfile::TempDir;

        let xz_data = two_component_archive();

        // Set up storage and mock DB
        let tmp_dir = TempDir::new().unwrap();
        let toolchain_dir = tmp_dir.path().join("toolchains");
        std::fs::create_dir_all(&toolchain_dir).unwrap();
        let fs = Box::new(FSStorage::new(toolchain_dir.to_str().unwrap()).unwrap()) as DynStorage;
        let storage = Arc::new(ToolchainStorage::new(fs));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_is_toolchain_component_stored_at()
            .returning(|_| Ok(false));
        mock_db
            .expect_add_toolchain_component()
            .times(2)
            .returning(|_, _, _, _, _| Ok(()));

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);

        let result = extract_and_store_components(
            &storage,
            &db,
            1,
            &ToolchainArchiveId {
                name: "rust",
                version: "1.0.0",
                date: "2024-01-15",
                target: "x86_64-unknown-linux-gnu",
            },
            Bytes::from(xz_data),
        )
        .await;

        assert!(result.is_ok(), "extraction failed: {result:?}");

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
    /// was stored at the path of the rustc component, which a target refers
    /// to if `in_use`.
    async fn extract_over_leftover(
        storage: &Arc<ToolchainStorage>,
        in_use: bool,
    ) -> Result<(), String> {
        storage
            .put_raw(&stored(RUSTC_COMPONENT), Bytes::from_static(b"leftover"))
            .await
            .unwrap();
        let mut mock_db = MockDb::new();
        mock_db
            .expect_is_toolchain_component_stored_at()
            .returning(move |path| Ok(in_use && path == RUSTC_COMPONENT));
        mock_db
            .expect_add_toolchain_component()
            .returning(|_, _, _, _, _| Ok(()));
        let db: Arc<dyn DbProvider> = Arc::new(mock_db);

        extract_and_store_components(
            storage,
            &db,
            1,
            &ToolchainArchiveId {
                name: "rust",
                version: "1.0.0",
                date: "2024-01-15",
                target: "x86_64-unknown-linux-gnu",
            },
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
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                            .to_string(),
                        hash: "abc123def456".to_string(),
                        size: 1024,
                        components: vec![],
                        status: ToolchainTargetStatus::Ready,
                    }],
                }))
            });

        let state = create_app_state(Arc::new(mock_db), None);
        let router = create_test_router(state);

        // rustup requests the .sha256 of the manifest before downloading
        // the manifest itself. This must return a valid SHA256 hash.
        let response = router
            .oneshot(
                Request::get("/api/v1/toolchains/dist/channel-rust-stable.toml.sha256")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let hash_content = String::from_utf8(body.to_vec()).unwrap();

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

    fn admin_db() -> MockDb {
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
    }

    fn stored(path: &str) -> StorageKey {
        StorageKey::from_stored(path.to_string())
    }

    fn fs_toolchain_storage(dir: &TempDir) -> Arc<ToolchainStorage> {
        let storage: DynStorage = Box::new(FSStorage::new(dir.path().to_str().unwrap()).unwrap());
        Arc::new(ToolchainStorage::new(storage))
    }

    /// A misspelled target must not count as the last target, which would
    /// delete the whole toolchain.
    #[tokio::test]
    async fn delete_unknown_target_of_single_target_toolchain_is_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock_db = admin_db();
        mock_db
            .expect_get_toolchain_by_version()
            .with(eq("rust"), eq("1.0.0"))
            .returning(|_, _| Ok(Some(ready_toolchain(&["x86_64-unknown-linux-gnu"]))));
        mock_db.expect_delete_toolchain().never();
        mock_db.expect_delete_toolchain_target().never();

        let state = create_app_state(Arc::new(mock_db), Some(fs_toolchain_storage(&temp_dir)));
        let response = create_test_router(state)
            .oneshot(
                Request::delete("/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnux")
                    .header(header::COOKIE, admin_cookie())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, response.status());
    }

    /// Stands in for a database error whose text must not reach the client.
    fn db_error() -> DbError {
        DbError::InitializationError("secret detail".to_string())
    }

    const ARCHIVE_PATH: &str = "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz";
    const UPLOAD_URL: &str = "/api/v1/toolchains?name=rust&version=1.0.0&target=x86_64-unknown-linux-gnu&date=2024-01-15";

    async fn send(state: AppStateData, request: Request<Body>) -> (StatusCode, String) {
        let response = create_test_router(state).oneshot(request).await.unwrap();
        let status = response.status();
        (status, error_detail(response).await)
    }

    fn upload_request() -> Request<Body> {
        Request::put(UPLOAD_URL)
            .header(header::COOKIE, admin_cookie())
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(sample_archive()))
            .unwrap()
    }

    fn delete_request(uri: &str) -> Request<Body> {
        Request::delete(uri)
            .header(header::COOKIE, admin_cookie())
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn upload_does_not_create_toolchain_when_archive_cannot_be_stored() {
        let temp_dir = TempDir::new().unwrap();
        let storage = fs_toolchain_storage(&temp_dir);
        storage
            .put_raw(&stored(ARCHIVE_PATH), Bytes::from_static(b"orphan"))
            .await
            .unwrap();
        let mut mock_db = admin_db();
        mock_db
            .expect_get_toolchain_by_version()
            .returning(|_, _| Ok(None));
        mock_db.expect_add_toolchain().never();
        mock_db.expect_add_toolchain_target().never();

        let (status, _) = send(
            create_app_state(Arc::new(mock_db), Some(storage)),
            upload_request(),
        )
        .await;

        assert_eq!(StatusCode::CONFLICT, status);
    }

    #[tokio::test]
    async fn upload_removes_archive_and_new_toolchain_when_target_cannot_be_added() {
        let temp_dir = TempDir::new().unwrap();
        let storage = fs_toolchain_storage(&temp_dir);
        let mut mock_db = admin_db();
        mock_db
            .expect_get_toolchain_by_version()
            .returning(|_, _| Ok(None));
        mock_db.expect_add_toolchain().returning(|_, _, _, _| Ok(1));
        mock_db
            .expect_add_toolchain_target()
            .returning(|_, _, _, _, _| Err(db_error()));
        mock_db
            .expect_delete_toolchain()
            .with(eq("rust"), eq("1.0.0"))
            .times(1)
            .returning(|_, _| Ok(()));

        let (status, result) = send(
            create_app_state(Arc::new(mock_db), Some(storage.clone())),
            upload_request(),
        )
        .await;

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status);
        assert!(!result.contains("secret detail"));
        assert!(!storage.exists(&stored(ARCHIVE_PATH)).await.unwrap());
    }

    #[tokio::test]
    async fn delete_toolchain_keeps_archives_when_database_delete_fails() {
        let temp_dir = TempDir::new().unwrap();
        let storage = fs_toolchain_storage(&temp_dir);
        storage
            .put_raw(&stored(ARCHIVE_PATH), Bytes::from_static(b"archive"))
            .await
            .unwrap();
        let mut mock_db = admin_db();
        mock_db
            .expect_get_toolchain_by_version()
            .returning(|_, _| Ok(Some(ready_toolchain(&["x86_64-unknown-linux-gnu"]))));
        mock_db
            .expect_delete_toolchain()
            .returning(|_, _| Err(db_error()));

        let (status, result) = send(
            create_app_state(Arc::new(mock_db), Some(storage.clone())),
            delete_request("/api/v1/toolchains/rust/1.0.0"),
        )
        .await;

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status);
        assert!(!result.contains("secret detail"));
        assert!(storage.exists(&stored(ARCHIVE_PATH)).await.unwrap());
    }

    #[tokio::test]
    async fn delete_target_keeps_archives_when_database_delete_fails() {
        let temp_dir = TempDir::new().unwrap();
        let storage = fs_toolchain_storage(&temp_dir);
        storage
            .put_raw(&stored(ARCHIVE_PATH), Bytes::from_static(b"archive"))
            .await
            .unwrap();
        let mut mock_db = admin_db();
        mock_db.expect_get_toolchain_by_version().returning(|_, _| {
            Ok(Some(ready_toolchain(&[
                "x86_64-unknown-linux-gnu",
                "aarch64-unknown-linux-gnu",
            ])))
        });
        mock_db
            .expect_delete_toolchain_target()
            .returning(|_, _, _| Err(db_error()));

        let (status, _) = send(
            create_app_state(Arc::new(mock_db), Some(storage.clone())),
            delete_request("/api/v1/toolchains/rust/1.0.0/targets/x86_64-unknown-linux-gnu"),
        )
        .await;

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status);
        assert!(storage.exists(&stored(ARCHIVE_PATH)).await.unwrap());
    }

    #[tokio::test]
    async fn database_errors_are_not_returned_to_the_client() {
        let mut mock_db = admin_db();
        mock_db
            .expect_get_toolchain_by_version()
            .returning(|_, _| Err(db_error()));
        let temp_dir = TempDir::new().unwrap();

        let (status, result) = send(
            create_app_state(Arc::new(mock_db), Some(fs_toolchain_storage(&temp_dir))),
            delete_request("/api/v1/toolchains/rust/1.0.0"),
        )
        .await;

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status);
        assert!(!result.contains("secret detail"));
    }

    #[tokio::test]
    async fn target_is_marked_failed_when_processing_panics() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_set_target_status()
            .with(eq(7), eq(ToolchainTargetStatus::Failed))
            .times(1)
            .returning(|_, _| Ok(()));

        spawn_target_processing(&BackgroundTasks::new(), Arc::new(mock_db), 7, async {
            panic!("extraction bug")
        })
        .await
        .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_waits_for_target_processing() {
        let tasks = BackgroundTasks::new();
        let finished = Arc::new(AtomicBool::new(false));
        let finished_in_task = finished.clone();

        spawn_target_processing(&tasks, Arc::new(MockDb::new()), 7, async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            finished_in_task.store(true, Ordering::SeqCst);
        });
        tasks.request_shutdown();
        tasks.wait_for_shutdown(Duration::from_secs(30)).await;

        assert!(finished.load(Ordering::SeqCst));
    }
}
