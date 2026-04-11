use std::fmt::Write;
use std::io::Read as _;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::put;
use axum::{Json, Router};
use bytes::Bytes;
use kellnr_appstate::{AppStateData, DbState, SettingsState, ToolchainStorageState};
use kellnr_db::{ChannelInfo, ToolchainWithTargets};
use kellnr_storage::toolchain_storage::ToolchainStorage;
use kellnr_web_ui::session::AdminUser;
use serde::{Deserialize, Serialize};
use tracing::trace;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

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

/// Creates the toolchain API routes (management endpoints)
pub fn create_api_routes(_state: AppStateData, max_size: usize) -> OpenApiRouter<AppStateData> {
    // Upload route needs custom body limit, so we merge it as a regular Router
    let upload_router: OpenApiRouter<AppStateData> = Router::new()
        .route(
            "/",
            put(upload_toolchain).layer(DefaultBodyLimit::max(max_size * 1_000_000)),
        )
        .into();

    OpenApiRouter::new()
        .routes(routes!(list_toolchains))
        .merge(upload_router)
        .routes(routes!(delete_toolchain))
        .routes(routes!(delete_toolchain_target))
        .routes(routes!(list_channels))
        .routes(routes!(set_channel))
}

/// Creates the toolchain distribution routes (download endpoints)
pub fn create_dist_routes(_state: AppStateData) -> OpenApiRouter<AppStateData> {
    // No authentication on dist routes — rustup does not support HTTP authentication
    OpenApiRouter::new()
        // Use a full segment parameter and parse the manifest filename in the handler
        // because Axum doesn't allow parameters in the middle of a path segment
        .routes(routes!(get_channel_manifest))
        .routes(routes!(download_archive))
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
    body: Bytes,
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
    let target_id = db
        .add_toolchain_target(toolchain_id, &params.target, &path, &hash, size)
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
    tokio::spawn(async move {
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
                            if let Err(status_err) =
                                bg_db.set_target_status(target_id, "failed").await
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

        match extract_and_store_components(
            &bg_storage,
            &bg_db,
            target_id,
            &bg_name,
            &bg_version,
            &bg_target,
            &bg_date,
            &archive_bytes,
        )
        .await
        {
            Ok(()) => {
                if let Err(e) = bg_db.set_target_status(target_id, "ready").await {
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
                if let Err(status_err) = bg_db.set_target_status(target_id, "failed").await {
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

    // Delete all archives (combined + individual components) from storage
    for target in &tc.targets {
        if let Err(e) = storage.delete(&target.storage_path).await {
            tracing::warn!("Failed to delete archive from storage: {e}");
        }
        for component in &target.components {
            if let Err(e) = storage.delete(&component.storage_path).await {
                tracing::warn!("Failed to delete component archive from storage: {e}");
            }
        }
    }

    // Delete toolchain from database (cascades to targets and components)
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
        (status = 404, description = "Toolchain not found"),
        (status = 503, description = "Storage not configured")
    ),
    security(("session_cookie" = []))
)]
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

    // Delete the archive and component archives from storage
    if let Some(t) = tc.targets.iter().find(|t| t.target == target) {
        if let Err(e) = storage.delete(&t.storage_path).await {
            tracing::warn!("Failed to delete archive from storage: {e}");
        }
        for component in &t.components {
            if let Err(e) = storage.delete(&component.storage_path).await {
                tracing::warn!("Failed to delete component archive from storage: {e}");
            }
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
        (status = 404, description = "Channel not found")
    ),
    security(("session_cookie" = []))
)]
async fn get_channel_manifest(
    State(db): DbState,
    State(settings): SettingsState,
    Path(manifest_file): Path<String>,
) -> Result<Response, StatusCode> {
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

    let toolchain = db
        .get_toolchain_by_channel(channel)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

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

/// Extract individual component archives from a combined toolchain archive.
///
/// The combined `rust-{version}-{target}.tar.xz` archive contains sub-component
/// directories (rustc, cargo, rust-std, etc.) each with their own `manifest.in`.
/// This function extracts each component into its own `.tar.xz` archive so rustup
/// can download them individually.
///
/// Uses two streamed passes through the archive.
/// First it reads metadata (`components`, `rust-installer-version`),
/// then it routes entries into per-component archives.
/// This keeps memory bounded while still being robust if metadata appears late.
#[expect(clippy::too_many_arguments)]
async fn extract_and_store_components(
    storage: &Arc<ToolchainStorage>,
    db: &Arc<dyn kellnr_db::DbProvider>,
    target_id: i64,
    name: &str,
    version: &str,
    target: &str,
    date: &str,
    archive_bytes: &[u8],
) -> Result<(), String> {
    use std::collections::HashMap;

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
    let mut builders: HashMap<String, tar::Builder<Vec<u8>>> = HashMap::new();
    for cn in &component_names {
        let mut builder = tar::Builder::new(Vec::new());
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
        let mut entry_data = Vec::new();
        entry
            .read_to_end(&mut entry_data)
            .map_err(|e| format!("Failed to read entry data: {e}"))?;

        let mut header = tar::Header::new_gnu();
        header.set_size(entry_data.len() as u64);
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
            .append_data(&mut header, &new_path, &entry_data[..])
            .map_err(|e| format!("Failed to append to tar: {e}"))?;
    }

    tracing::debug!(
        components = ?component_names,
        "Extracted {} components in two streamed passes, compressing and storing",
        component_names.len()
    );

    // Finish, compress, and store each component archive.
    // Track stored paths so we can clean up on failure (e.g. if the target
    // was deleted concurrently and the DB insert fails with an FK violation).
    let mut stored_paths: Vec<String> = Vec::new();

    for component_name in &component_names {
        let builder = builders
            .remove(component_name)
            .ok_or_else(|| format!("Missing builder for {component_name}"))?;

        let component_tar = builder
            .into_inner()
            .map_err(|e| format!("Failed to finish tar for {component_name}: {e}"))?;

        let mut xz_encoder = xz2::write::XzEncoder::new(Vec::new(), 6);
        std::io::Write::write_all(&mut xz_encoder, &component_tar)
            .map_err(|e| format!("Failed to compress {component_name}: {e}"))?;
        let component_xz = xz_encoder
            .finish()
            .map_err(|e| format!("Failed to finish xz for {component_name}: {e}"))?;

        let component_hash = sha256::digest(&component_xz[..]);
        let component_size = component_xz.len() as i64;
        let component_storage_path =
            ToolchainStorage::component_storage_path(date, component_name, version, target);

        storage
            .put_raw(&component_storage_path, Bytes::from(component_xz))
            .await
            .map_err(|e| format!("Failed to store component {component_name}: {e}"))?;
        stored_paths.push(component_storage_path.clone());

        if let Err(e) = db
            .add_toolchain_component(
                target_id,
                component_name,
                &component_storage_path,
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
            path = component_storage_path,
            size = component_size,
            "Stored component archive"
        );
    }

    Ok(())
}

/// Append a small metadata file (rust-installer-version, components) to a tar builder.
fn append_metadata_file(
    builder: &mut tar::Builder<Vec<u8>>,
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
    let base_url = format!(
        "{}://{}:{}/api/v1/toolchains/dist",
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

    // [pkg.rust] — the meta-package for the combined archive
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
        .filter(|t| t.status == "ready")
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

    // Individual component packages — rustup downloads these one by one
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
    use std::borrow::Cow;
    use std::io::Cursor;
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
    use kellnr_db::{
        DbProvider, ToolchainComponentInfo, ToolchainTargetInfo, ToolchainWithTargets,
    };
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
    fn create_app_state(
        db: Arc<dyn DbProvider>,
        toolchain_storage: Option<Arc<ToolchainStorage>>,
    ) -> AppStateData {
        let settings = kellnr_settings::test_settings();
        let kellnr_storage =
            Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
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
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

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
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

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
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

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
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

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
            .with(eq("user_session"))
            .returning(|_| Ok(("user".to_string(), false)));

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
                    storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                        .to_string(),
                    hash: "abc123".to_string(),
                    size: 1024,
                    components: vec![],
                    status: "ready".to_string(),
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
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));
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
                            storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                                .to_string(),
                            hash: "abc123".to_string(),
                            size: 1024,
                            components: vec![],
                            status: "ready".to_string(),
                        },
                        ToolchainTargetInfo {
                            id: 2,
                            target: "aarch64-unknown-linux-gnu".to_string(),
                            storage_path: "2024-01-15/rust-1.0.0-aarch64-unknown-linux-gnu.tar.xz"
                                .to_string(),
                            hash: "def456".to_string(),
                            size: 2048,
                            components: vec![],
                            status: "ready".to_string(),
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
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                            .to_string(),
                        hash: "abc123".to_string(),
                        size: 1024,
                        components: vec![],
                        status: "ready".to_string(),
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
                            storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                                .to_string(),
                            hash: "abc123".to_string(),
                            size: 1024,
                            components: vec![],
                            status: "ready".to_string(),
                        },
                        ToolchainTargetInfo {
                            id: 2,
                            target: "aarch64-unknown-linux-gnu".to_string(),
                            storage_path: "2024-01-15/rust-1.0.0-aarch64-unknown-linux-gnu.tar.xz"
                                .to_string(),
                            hash: "def456".to_string(),
                            size: 2048,
                            components: vec![],
                            status: "ready".to_string(),
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
                Request::delete("/api/v1/toolchains/rust/99.99.99")
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
            .with(eq("admin_session"))
            .returning(|_| Ok(("admin".to_string(), true)));

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

        let result: ToolchainResponse = parse_response(response).await;
        assert!(!result.success);
        assert!(result.message.unwrap().contains("not configured"));
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
                        storage_path: "2024-01-15/rust-1.0.0-x86_64-unknown-linux-gnu.tar.xz"
                            .to_string(),
                        hash: "abc123".to_string(),
                        size: 1024,
                        components: vec![],
                        status: "ready".to_string(),
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
                Request::put("/api/v1/toolchains/channels/stable")
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
                        status: "ready".to_string(),
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
                        status: "ready".to_string(),
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

    #[tokio::test]
    async fn test_extract_components_from_archive() {
        use std::io::Write;

        use tempfile::TempDir;

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
        let xz_data = xz_encoder.finish().unwrap();

        // Set up storage and mock DB
        let tmp_dir = TempDir::new().unwrap();
        let toolchain_dir = tmp_dir.path().join("toolchains");
        std::fs::create_dir_all(&toolchain_dir).unwrap();
        let fs = Box::new(FSStorage::new(toolchain_dir.to_str().unwrap()).unwrap()) as DynStorage;
        let storage = Arc::new(ToolchainStorage::new(fs));

        let mut mock_db = MockDb::new();
        mock_db
            .expect_add_toolchain_component()
            .times(2)
            .returning(|_, _, _, _, _| Ok(()));

        let db: Arc<dyn DbProvider> = Arc::new(mock_db);

        let result = extract_and_store_components(
            &storage,
            &db,
            1,
            "rust",
            "1.0.0",
            "x86_64-unknown-linux-gnu",
            "2024-01-15",
            &xz_data,
        )
        .await;

        assert!(result.is_ok(), "extraction failed: {result:?}");

        // Verify component archives were stored
        let rustc_exists = storage
            .exists("2024-01-15/rustc-1.0.0-x86_64-unknown-linux-gnu.tar.xz")
            .await
            .unwrap();
        assert!(rustc_exists, "rustc component archive not found in storage");

        let cargo_exists = storage
            .exists("2024-01-15/cargo-1.0.0-x86_64-unknown-linux-gnu.tar.xz")
            .await
            .unwrap();
        assert!(cargo_exists, "cargo component archive not found in storage");

        // Verify the component archive contains rust-installer-version and components files
        let rustc_archive = storage
            .get("2024-01-15/rustc-1.0.0-x86_64-unknown-linux-gnu.tar.xz")
            .await
            .unwrap();
        let mut xz_dec = xz2::read::XzDecoder::new(&rustc_archive[..]);
        let mut tar_bytes = Vec::new();
        xz_dec.read_to_end(&mut tar_bytes).unwrap();
        let mut archive = tar::Archive::new(Cursor::new(&tar_bytes));
        let entry_names: Vec<String> = archive
            .entries()
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path().unwrap().to_string_lossy().to_string())
            .collect();
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
                        status: "ready".to_string(),
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
}
