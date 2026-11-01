use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::crate_data::{CRATESIO_INDEX, CrateData};
use kellnr_common::crate_dependents::CrateDependents;
use kellnr_common::crate_overview::CrateOverview;
use kellnr_common::name_or_description::NameOrDescription;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::error::DbError;
use kellnr_db::{CrateSort, CrateTagFilter, DbProvider};
use kellnr_docs::upload::upload_dir_and_prune;
use kellnr_http::authz::{self, can_read_crate, crate_visibility};
use kellnr_http::error::ApiError;
use kellnr_http::identity::{AdminUser, AnyUser, Identity, SessionUser};
use kellnr_http::state::{AppState, DbState, ProxyClientState, SettingsProvState, SettingsState};
use kellnr_settings::{
    ConfigSource, Provenance, Settings, SettingsProv, SourceMap, cli_flag_map, compile_time_config,
    erased_serde, leaf_label, sources_from_prov,
};
use kellnr_storage::docs_storage::DocsName;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use utoipa::ToSchema;

use crate::pagination::{PageParams, PageRequest};

/// Settings response, the flattened `Settings`, the per-leaf source map
/// derived from the `SettingsProv`, and the compiled-in defaults the UI
/// uses to show "default: X" next to overridden values.
///
/// Secret leaves are redacted in `settings`, `defaults` and `leaves`: a set
/// secret is replaced by [`REDACTED`], an unset one is `null`.
///
/// `leaves` is the new dynamic schema: one entry per `SettingsProv` leaf with
/// the value, default, source, type, secret flag, optional CLI flag string
/// (auto-derived from clap) and optional label override. The Vue UI iterates
/// over this list instead of hand-maintaining a parallel per-leaf table.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SettingsResponse {
    #[serde(flatten)]
    #[schema(value_type = Settings)]
    pub settings: serde_json::Map<String, serde_json::Value>,
    #[schema(inline)]
    pub sources: SourceMap,
    #[schema(value_type = Settings)]
    pub defaults: serde_json::Value,
    pub leaves: Vec<LeafMeta>,
}

/// Placeholder sent instead of the value of a configured secret.
pub const REDACTED: &str = "********";

/// Per-leaf metadata for the dynamic UI. All fields except `value`/`source`
/// are stable per build (or, in the case of `default`, per binary), so the UI
/// can treat the response as both data and schema.
#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
pub struct LeafMeta {
    /// Dotted path, e.g. `"registry.data_dir"`.
    pub key: String,
    /// Active value as a JSON value (boolean, number, string, array, null).
    pub value: serde_json::Value,
    /// Compiled-in default value in the same shape as `value`.
    pub default: serde_json::Value,
    /// Where the active value came from.
    pub source: ConfigSource,
    /// JSON-flavoured type tag the UI uses to pick a renderer.
    #[serde(rename = "type")]
    pub kind: LeafKind,
    /// `true` for `#[configurable(secret)]` leaves, the UI masks the value.
    pub secret: bool,
    /// CLI flag string (e.g. `"--registry-data-dir, -d"`) when one exists.
    pub cli_flag: Option<String>,
    /// Optional display label override; `None` means the UI humanizes the key.
    pub label: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LeafKind {
    Boolean,
    Number,
    String,
    Array,
}

/// Build the leaf metadata vector by walking `SettingsProv` once and joining
/// with `Settings::default()` (for defaults) and `cli_flag_map()` (for flags).
fn build_leaves(prov: &SettingsProv) -> Vec<LeafMeta> {
    let defaults = Settings::default();
    let defaults_json =
        serde_json::to_value(&defaults).expect("Settings serializes as JSON for the defaults map");
    let flag_map = cli_flag_map();

    let mut leaves: Vec<LeafMeta> = Vec::new();
    prov.walk_leaves("", &mut |path, value, category, secret| {
        // Re-serialize the type-erased value to JSON so the UI gets the same
        // shape as the surrounding `settings` block.
        let Ok(value_json) = json_from_erased(value) else {
            return;
        };
        let mut leaf_default = lookup_default(&defaults_json, path);
        let mut value_json = value_json;
        if secret {
            value_json = redact_secret(&value_json);
            leaf_default = redact_secret(&leaf_default);
        }
        let kind = LeafKind::from_json(&value_json);
        leaves.push(LeafMeta {
            key: path.to_string(),
            value: value_json,
            default: leaf_default,
            source: category.into(),
            kind,
            secret,
            cli_flag: flag_map.get(path).cloned(),
            label: leaf_label(path).map(String::from),
        });
    });
    // Stable order: alphabetic by dotted path. The UI groups by section
    // prefix anyway, so any consistent order is fine, alphabetic keeps the
    // response diff-friendly across runs.
    leaves.sort_by(|a, b| a.key.cmp(&b.key));
    leaves
}

/// Replace a secret value by [`REDACTED`], or by `null` if it is unset or empty,
/// so the UI can still tell "configured" from "not configured".
fn redact_secret(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Null => serde_json::Value::Null,
        serde_json::Value::String(s) if s.is_empty() => serde_json::Value::Null,
        _ => serde_json::Value::String(REDACTED.to_string()),
    }
}

/// Serialize `settings` to JSON and redact every secret leaf of `leaves`.
/// Secrets that are not serialized at all (`skip_serializing`) are skipped.
fn redacted_settings_json(
    settings: &Settings,
    leaves: &[LeafMeta],
) -> Result<serde_json::Value, serde_json::Error> {
    let mut json = serde_json::to_value(settings)?;
    for leaf in leaves.iter().filter(|leaf| leaf.secret) {
        let pointer = format!("/{}", leaf.key.replace('.', "/"));
        if let Some(value) = json.pointer_mut(&pointer) {
            *value = redact_secret(value);
        }
    }
    Ok(json)
}

/// Serialize an erased value into a `serde_json::Value` via the same shim
/// pattern used by `kellnr/src/config_printer.rs::toml_from_erased`.
fn json_from_erased(
    value: &dyn erased_serde::Serialize,
) -> Result<serde_json::Value, serde_json::Error> {
    struct Erased<'a>(&'a dyn erased_serde::Serialize);
    impl Serialize for Erased<'_> {
        fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
            erased_serde::serialize(self.0, ser)
        }
    }
    serde_json::to_value(Erased(value))
}

/// Look up `dotted_path` inside a JSON-serialized `Settings` and return the
/// matching leaf. Returns `Null` if the path is missing (shouldn't happen for
/// any walked leaf, but the JSON null keeps the UI's renderer happy).
fn lookup_default(defaults_json: &serde_json::Value, dotted_path: &str) -> serde_json::Value {
    let mut current = defaults_json;
    for segment in dotted_path.split('.') {
        let Some(next) = current.get(segment) else {
            return serde_json::Value::Null;
        };
        current = next;
    }
    current.clone()
}

impl LeafKind {
    fn from_json(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Bool(_) => Self::Boolean,
            serde_json::Value::Number(_) => Self::Number,
            serde_json::Value::Array(_) => Self::Array,
            // String, null, object → fall back to string. `null` represents
            // `Option::None`; objects shouldn't appear (only leaves are
            // visited), but if they do the UI prints them via String(_) too.
            _ => Self::String,
        }
    }
}

/// Get Kellnr settings (admin only)
#[utoipa::path(
    get,
    path = "/settings",
    tag = "ui",
    responses(
        (status = 200, description = "Kellnr settings with source tracking", body = SettingsResponse),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
#[allow(clippy::unused_async)] // part of the router
pub async fn settings(
    _user: AdminUser,
    State(settings): SettingsState,
    State(prov): SettingsProvState,
) -> Result<Json<SettingsResponse>, ApiError> {
    let leaves = build_leaves(&prov);
    let serialization_error = |e: serde_json::Error| {
        error!("Failed to serialize settings: {e}");
        ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
    };
    let serde_json::Value::Object(settings) =
        redacted_settings_json(&settings, &leaves).map_err(serialization_error)?
    else {
        return Err(ApiError::from(StatusCode::INTERNAL_SERVER_ERROR));
    };
    let defaults =
        redacted_settings_json(&Settings::default(), &leaves).map_err(serialization_error)?;

    Ok(Json(SettingsResponse {
        sources: sources_from_prov(&prov),
        settings,
        defaults,
        leaves,
    }))
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct DocsEnabledResponse {
    pub enabled: bool,
}

/// Check if documentation generation is enabled
#[utoipa::path(
    get,
    path = "/docs_enabled",
    tag = "ui",
    responses(
        (status = 200, description = "Documentation generation status", body = DocsEnabledResponse)
    )
)]
#[allow(clippy::unused_async)] // part of the router
pub async fn docs_enabled(State(settings): SettingsState) -> Json<DocsEnabledResponse> {
    Json(DocsEnabledResponse {
        enabled: settings.docs.enabled,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct SponsorLinksEnabledResponse {
    pub enabled: bool,
}

/// Check if the web UI shows links to sponsor Kellnr
#[utoipa::path(
    get,
    path = "/sponsor_links_enabled",
    tag = "ui",
    responses(
        (status = 200, description = "Sponsor links status", body = SponsorLinksEnabledResponse)
    )
)]
#[allow(clippy::unused_async)] // part of the router
pub async fn sponsor_links_enabled(
    State(settings): SettingsState,
) -> Json<SponsorLinksEnabledResponse> {
    Json(SponsorLinksEnabledResponse {
        enabled: settings.registry.sponsor_links_enabled,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema)]
pub struct KellnrVersion {
    pub version: String,
}

/// Get Kellnr version
#[utoipa::path(
    get,
    path = "/version",
    tag = "ui",
    responses(
        (status = 200, description = "Kellnr version", body = KellnrVersion)
    )
)]
#[allow(clippy::unused_async)] // part of the router
pub async fn kellnr_version() -> Json<KellnrVersion> {
    Json(KellnrVersion {
        version: compile_time_config::KELLNR_COMPTIME__VERSION.to_string(),
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema, utoipa::IntoParams)]
pub struct CratesParams {
    cache: Option<bool>,
    sort: Option<CrateSort>,
    /// Only crates with this keyword
    keyword: Option<String>,
    /// Only crates in this category
    category: Option<String>,
}

/// Logs a failed database query and returns a 500.
fn db_error(e: &DbError) -> ApiError {
    error!("Database query failed: {e}");
    ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema)]
pub struct Pagination {
    crates: Vec<CrateOverview>,
    page_size: u64,
    /// Number of the page, starting at 0
    page: u64,
    /// Number of crates on all pages
    total: u64,
}

/// Get paginated list of crates
#[utoipa::path(
    get,
    path = "/crates",
    tag = "ui",
    params(CratesParams, PageParams),
    responses(
        (status = 200, description = "Paginated crate list", body = Pagination),
        (status = 400, description = "Page out of range")
    )
)]
pub async fn crates(
    Query(params): Query<CratesParams>,
    page: PageRequest,
    State(db): DbState,
    user: Option<AnyUser>,
) -> Result<Json<Pagination>, ApiError> {
    let cache = params.cache.unwrap_or(false);
    let visibility = crate_visibility(user.as_deref());
    let sort = params.sort.unwrap_or_default();
    let tags = CrateTagFilter {
        keyword: params.keyword.filter(|k| !k.is_empty()),
        category: params.category.filter(|c| !c.is_empty()),
    };
    let crates = db
        .get_crate_overview_list(
            page.size().get(),
            page.offset(),
            cache,
            &visibility,
            sort,
            &tags,
        )
        .await
        .map_err(|e| db_error(&e))?;
    let total = db
        .count_crate_overview_list(cache, &visibility, &tags)
        .await
        .map_err(|e| db_error(&e))?;

    Ok(Json(Pagination {
        crates,
        page_size: page.size().get(),
        page: page.page(),
        total,
    }))
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema, utoipa::IntoParams)]
pub struct SearchParams {
    name: NameOrDescription,
    cache: Option<bool>,
}

/// Search for crates by name and description
#[utoipa::path(
    get,
    path = "/search",
    tag = "ui",
    params(SearchParams, PageParams),
    responses(
        (status = 200, description = "Paginated search results", body = Pagination),
        (status = 400, description = "Page out of range")
    )
)]
pub async fn search(
    Query(params): Query<SearchParams>,
    page: PageRequest,
    State(db): DbState,
    user: Option<AnyUser>,
) -> Result<Json<Pagination>, ApiError> {
    let cache = params.cache.unwrap_or(false);
    let visibility = crate_visibility(user.as_deref());
    let crates = db
        .search_in_crate_name_and_description(
            params.name.as_str(),
            page.size().get(),
            page.offset(),
            cache,
            &visibility,
        )
        .await
        .map_err(|e| db_error(&e))?;
    let total = db
        .count_by_crate_name_and_description(params.name.as_str(), cache, &visibility)
        .await
        .map_err(|e| db_error(&e))?;

    Ok(Json(Pagination {
        crates,
        page_size: page.size().get(),
        page: page.page(),
        total,
    }))
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema, utoipa::IntoParams)]
pub struct CrateDataParams {
    name: OriginalName,
}

/// Get detailed crate data
#[utoipa::path(
    get,
    path = "/crate_data",
    tag = "ui",
    params(CrateDataParams),
    responses(
        (status = 200, description = "Crate details", body = CrateData),
        (status = 404, description = "Crate not found or download-restricted without access")
    )
)]
pub async fn crate_data(
    Query(params): Query<CrateDataParams>,
    State(db): DbState,
    user: Option<AnyUser>,
) -> Result<Json<CrateData>, ApiError> {
    let index_name = CanonicalName::from(params.name);
    // Same response as a missing crate to not reveal that a restricted crate exists.
    match can_read_crate(&index_name, user.as_deref(), &db).await {
        Ok(true) => {}
        Ok(false) => return Err(StatusCode::NOT_FOUND.into()),
        Err(e) => {
            error!("Failed to check access to crate {index_name}: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR.into());
        }
    }
    let mut cd = db.get_crate_data(&index_name).await.map_err(|e| match e {
        DbError::CrateNotFound(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;
    hide_restricted_dep_info(&mut cd, user.as_deref(), &db)
        .await
        .map_err(|e| {
            error!("Failed to check access to the dependencies of {index_name}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(cd))
}

/// Removes the description and latest version of dependencies that are
/// download-restricted crates `user` cannot access, so that a crate page does
/// not reveal them.
async fn hide_restricted_dep_info(
    cd: &mut CrateData,
    user: Option<&Identity>,
    db: &Arc<dyn DbProvider>,
) -> Result<(), DbError> {
    let mut readable: HashMap<String, bool> = HashMap::new();
    for dep in cd
        .versions
        .iter_mut()
        .flat_map(|v| v.dependencies.iter_mut())
    {
        if dep.registry.as_deref() == Some(CRATESIO_INDEX) {
            continue;
        }
        // A name no crate can have cannot be restricted
        let Ok(name) = CanonicalName::try_from(dep.name.as_str()) else {
            continue;
        };
        let can_read = if let Some(can_read) = readable.get(name.as_str()) {
            *can_read
        } else {
            let can_read = can_read_crate(&name, user, db).await?;
            readable.insert(name.to_string(), can_read);
            can_read
        };
        if !can_read {
            dep.description = None;
            dep.latest_version = None;
            dep.outdated = false;
        }
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema, utoipa::IntoParams)]
pub struct CrateDependentsParams {
    name: OriginalName,
}

/// Get the crates whose latest version depends on a crate
#[utoipa::path(
    get,
    path = "/crate_dependents",
    tag = "ui",
    params(CrateDependentsParams),
    responses(
        (status = 200, description = "Dependents of the crate", body = CrateDependents),
        (status = 404, description = "Crate not found or download-restricted without access")
    )
)]
pub async fn crate_dependents(
    Query(params): Query<CrateDependentsParams>,
    State(db): DbState,
    user: Option<AnyUser>,
) -> Result<Json<CrateDependents>, ApiError> {
    let index_name = CanonicalName::from(params.name);
    // Same response as a missing crate to not reveal that a restricted crate exists.
    match can_read_crate(&index_name, user.as_deref(), &db).await {
        Ok(true) => {}
        Ok(false) => return Err(StatusCode::NOT_FOUND.into()),
        Err(e) => {
            error!("Failed to check access to crate {index_name}: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR.into());
        }
    }
    let mut cd = db
        .get_crate_dependents(&index_name)
        .await
        .map_err(|e| match e {
            DbError::CrateNotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;
    hide_restricted_dependents(&mut cd, user.as_deref(), &db)
        .await
        .map_err(|e| {
            error!("Failed to check access to the dependents of {index_name}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(cd))
}

/// Removes the dependents that are download-restricted crates `user` cannot
/// access and only counts them, so that the crate page shows that they exist
/// but nothing about them.
async fn hide_restricted_dependents(
    cd: &mut CrateDependents,
    user: Option<&Identity>,
    db: &Arc<dyn DbProvider>,
) -> Result<(), DbError> {
    let mut visible = Vec::with_capacity(cd.dependents.len());
    for dependent in std::mem::take(&mut cd.dependents) {
        let can_read = match CanonicalName::try_from(dependent.name.as_str()) {
            Ok(name) => can_read_crate(&name, user, db).await?,
            Err(_) => false,
        };
        if can_read {
            visible.push(dependent);
        } else {
            cd.restricted += 1;
        }
    }
    cd.dependents = visible;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema, utoipa::IntoParams)]
pub struct CratesIoDataParams {
    name: OriginalName,
}

/// Get crate data from crates.io
#[utoipa::path(
    get,
    path = "/cratesio_data",
    tag = "ui",
    params(CratesIoDataParams),
    responses(
        (
            status = 200,
            description = "The JSON answer of the crates.io API for the crate \
                (`GET https://crates.io/api/v1/crates/{name}`), passed through unchanged",
            body = String
        ),
        (status = 404, description = "Crate not found"),
        (status = 502, description = "crates.io failed or cannot be reached")
    )
)]
pub async fn cratesio_data(
    State(settings): SettingsState,
    State(client): ProxyClientState,
    Query(params): Query<CratesIoDataParams>,
) -> Result<String, ApiError> {
    let url = settings
        .proxy
        .api
        .join(&params.name)
        .map_err(internal_error)?;

    let resp = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| {
            error!("Failed to get crates.io data: {e}");
            StatusCode::BAD_GATEWAY
        })?;

    match resp.status() {
        StatusCode::OK => resp.text().await.map_err(|e| {
            error!("Failed to read crates.io data: {e}");
            StatusCode::BAD_GATEWAY.into()
        }),
        StatusCode::NOT_FOUND => Err(StatusCode::NOT_FOUND.into()),
        status => {
            error!("Failed to get crates.io data: {status}");
            Err(StatusCode::BAD_GATEWAY.into())
        }
    }
}

/// Helper function to delete crate versions from storage, db, and docs.
/// If `versions` is `None`, all versions of the crate are deleted.
///
/// The crate file is deleted before the database entry, so that a failure
/// cannot leave a file behind that no database entry refers to. A failed
/// version does not stop the deletion of the others.
async fn delete_crate_versions_impl(
    state: &kellnr_http::state::AppStateData,
    name: &OriginalName,
    versions: Option<Vec<Version>>,
) -> Result<(), ApiError> {
    let canonical_name = name.to_canonical();
    // Crate files are stored under the name the crate was first published
    // with, which may differ in case or separators from the requested name.
    let original_name = state
        .db
        .get_crate_original_name(&canonical_name)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("Crate not found: {name}")))?;

    let versions_to_delete = if let Some(v) = versions {
        v
    } else {
        state
            .db
            .get_crate_meta_list(&canonical_name)
            .await?
            .iter()
            .filter_map(|cm| {
                Version::try_from(cm.version.as_str())
                    .inspect_err(|_| {
                        warn!("Skipping invalid version {} of crate {name}", cm.version);
                    })
                    .ok()
            })
            .collect()
    };

    let mut first_error = None;
    for version in &versions_to_delete {
        if let Err(e) = delete_one_version(state, &original_name, version).await {
            first_error.get_or_insert(e);
        }
    }

    first_error.map_or(Ok(()), Err)
}

async fn delete_one_version(
    state: &kellnr_http::state::AppStateData,
    name: &OriginalName,
    version: &Version,
) -> Result<(), ApiError> {
    let canonical_name = name.to_canonical();

    // A missing file is fine, e.g. when an earlier deletion failed after it.
    let exists = state
        .crate_storage
        .exists(name, version)
        .await
        .map_err(|e| {
            error!("Failed to check crate {name}-{version} in storage: {e}");
            ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
        })?;
    if exists && let Err(e) = state.crate_storage.delete(name, version).await {
        error!("Failed to delete crate {name}-{version} from storage: {e}");
        return Err(ApiError::from(StatusCode::INTERNAL_SERVER_ERROR));
    }

    state
        .db
        .delete_crate(&canonical_name, version)
        .await
        .inspect_err(|e| error!("Failed to delete crate {name}-{version} from database: {e}"))?;

    // The version may be published again with another license
    state
        .download_filters
        .registry
        .licenses()
        .forget(&canonical_name, version);

    if let Err(e) = kellnr_docs::delete(name, version, &state.docs_storage).await {
        error!("Failed to delete docs of crate {name}-{version}: {e}");
        return Err(ApiError::from(StatusCode::INTERNAL_SERVER_ERROR));
    }

    Ok(())
}

/// Delete a specific version of a crate (path parameter version)
#[utoipa::path(
    delete,
    path = "/crates/{name}/{version}",
    tag = "ui",
    params(
        ("name" = String, Path, description = "Crate name"),
        ("version" = String, Path, description = "Version to delete")
    ),
    responses(
        (status = 200, description = "Crate version deleted"),
        (status = 400, description = "Invalid parameters"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_crate_version(
    Path((name, version)): Path<(String, String)>,
    _user: AdminUser,
    State(state): AppState,
) -> Result<(), ApiError> {
    let name = OriginalName::try_from(name.as_str())?;
    let version = Version::try_from(version.as_str())?;

    delete_crate_versions_impl(&state, &name, Some(vec![version])).await
}

/// Delete all versions of a crate (path parameter version)
#[utoipa::path(
    delete,
    path = "/crates/{name}",
    tag = "ui",
    params(
        ("name" = String, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "All crate versions deleted"),
        (status = 400, description = "Invalid parameters"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_crate_all(
    Path(name): Path<String>,
    _user: AdminUser,
    State(state): AppState,
) -> Result<(), ApiError> {
    let name = OriginalName::try_from(name.as_str())?;

    delete_crate_versions_impl(&state, &name, None).await
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct Statistic {
    pub num_crates: u64,
    pub num_crate_versions: u64,
    pub num_crate_downloads: u64,
    pub num_proxy_crates: u64,
    pub num_proxy_crate_versions: u64,
    pub num_proxy_crate_downloads: u64,
    pub top_crates: TopCrates,
    pub last_updated_crate: Option<(OriginalName, Version)>,
    pub proxy_enabled: bool,
}

/// The most downloaded crates, `None` if there are fewer crates.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct TopCrates {
    pub first: Option<(String, u64)>,
    pub second: Option<(String, u64)>,
    pub third: Option<(String, u64)>,
}

/// Get registry statistics
#[utoipa::path(
    get,
    path = "/statistics",
    tag = "ui",
    responses(
        (status = 200, description = "Registry statistics", body = Statistic)
    )
)]
pub async fn statistic(
    State(db): DbState,
    State(settings): SettingsState,
    user: Option<AnyUser>,
) -> Result<Json<Statistic>, ApiError> {
    let num_crates = db
        .get_total_unique_crates()
        .await
        .map_err(|e| db_error(&e))?;
    let num_crate_versions = db
        .get_total_crate_versions()
        .await
        .map_err(|e| db_error(&e))?;
    let num_crate_downloads = db.get_total_downloads().await.map_err(|e| db_error(&e))?;
    // Crate names are only listed if the caller may see them. The totals
    // are aggregates and do not reveal individual crates.
    let visibility = crate_visibility(user.as_deref());
    let mut tops = db
        .get_top_crates_downloads(3, &visibility)
        .await
        .map_err(|e| db_error(&e))?
        .into_iter();
    let num_proxy_crates = db
        .get_total_unique_cached_crates()
        .await
        .map_err(|e| db_error(&e))?;
    let num_proxy_crate_versions = db
        .get_total_cached_crate_versions()
        .await
        .map_err(|e| db_error(&e))?;
    let num_proxy_crate_downloads = db
        .get_total_cached_downloads()
        .await
        .map_err(|e| db_error(&e))?;
    let last_updated_crate = db
        .get_last_updated_crate(&visibility)
        .await
        .map_err(|e| db_error(&e))?;

    Ok(Json(Statistic {
        num_crates,
        num_crate_versions,
        num_crate_downloads,
        num_proxy_crates,
        num_proxy_crate_versions,
        num_proxy_crate_downloads,
        top_crates: TopCrates {
            first: tops.next(),
            second: tops.next(),
            third: tops.next(),
        },
        last_updated_crate,
        proxy_enabled: settings.proxy.enabled,
    }))
}

/// Parameters for triggering a documentation build
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema, utoipa::IntoParams)]
pub struct BuildParams {
    /// Package name
    package: OriginalName,
    /// Package version
    version: Version,
}

/// Trigger documentation build for a crate
///
/// Add a crate version to the documentation build queue.
/// Requires ownership of the crate without read-only restriction or admin access.
#[utoipa::path(
    post,
    path = "/builds",
    tag = "docs",
    params(BuildParams),
    responses(
        (status = 200, description = "Build queued successfully"),
        (status = 400, description = "Crate or version does not exist"),
        (status = 401, description = "Not logged in"),
        (status = 403, description = "Not an owner of the crate or read-only")
    ),
    security(("session_cookie" = []))
)]
pub async fn build_rustdoc(
    Query(params): Query<BuildParams>,
    State(state): AppState,
    user: SessionUser,
) -> Result<(), ApiError> {
    if !state.settings.docs.enabled {
        return Err(ApiError::bad_request(
            "Documentation generation is disabled",
        ));
    }

    let canonical_name = CanonicalName::from(params.package);
    let db = state.db;
    let version = params.version;

    // If the user is a non-read-only owner of the crate or any admin user,
    // the build operation is allowed. This comes before the version check, so
    // that others cannot tell which crates and versions exist: they get the
    // same error for a missing crate.
    authz::require_owner(&canonical_name, &user, &db).await?;
    authz::require_writable(&user)?;

    // Check if crate with the version exists.
    let Some(original_name) = db
        .get_original_name(&canonical_name, &version)
        .await
        .map_err(internal_error)?
    else {
        return Err(ApiError::bad_request(format!(
            "Crate with version does not exist: {canonical_name}-{version}"
        )));
    };

    // Add to build queue
    db.add_doc_queue(
        &original_name,
        &version,
        &state
            .crate_storage
            .create_rand_doc_queue_path()
            .await
            .map_err(internal_error)?,
    )
    .await
    .map_err(internal_error)?;

    Ok(())
}

/// Logs `e` and returns a 500 without leaking details to the client.
fn internal_error(e: impl std::fmt::Display) -> StatusCode {
    error!("Internal server error: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

/// Response for [`migrate_docs_storage`].
#[derive(Serialize, Deserialize, ToSchema)]
pub struct MigrateDocsStorageResponse {
    /// `false` when there was no local docs directory to migrate from.
    pub started: bool,
    /// Number of crate/version pairs found under the local docs directory.
    pub crate_versions_found: usize,
}

/// Copy locally-stored docs into the configured S3/GCS docs bucket
///
/// One-time backfill for existing deployments that switch `s3.docs_bucket`/
/// `gcs.docs_bucket` from unset to a bucket name: copies every file already
/// on local disk (both auto-generated and manually-published docs use the
/// same on-disk layout) into the now-configured backend. Non-destructive —
/// local files are left in place, so this is safe to re-run. Runs in the
/// background since a large registry can take minutes to copy; admin-only.
#[utoipa::path(
    post,
    path = "/migrate-storage",
    tag = "docs",
    responses(
        (status = 200, description = "No local docs found to migrate", body = MigrateDocsStorageResponse),
        (status = 202, description = "Migration started in the background", body = MigrateDocsStorageResponse),
        (status = 400, description = "Docs are still backed by local disk, nothing to migrate to"),
        (status = 401, description = "Not authorized"),
        (status = 403, description = "Not an admin")
    ),
    security(("session_cookie" = []))
)]
pub async fn migrate_docs_storage(
    State(state): AppState,
    _admin: AdminUser,
) -> Result<(StatusCode, Json<MigrateDocsStorageResponse>), ApiError> {
    if state.settings.docs_backend_is_local() {
        return Err(ApiError::bad_request(
            "Docs are stored locally, there is nothing to migrate",
        ));
    }

    let docs_path = PathBuf::from(state.settings.docs_path());
    let crate_versions = match list_local_crate_versions(&docs_path).await {
        Ok(crate_versions) => crate_versions,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            error!("Failed to read local docs directory {docs_path:?}: {e}");
            return Err(ApiError::from(StatusCode::INTERNAL_SERVER_ERROR));
        }
    };

    if crate_versions.is_empty() {
        return Ok((
            StatusCode::OK,
            Json(MigrateDocsStorageResponse {
                started: false,
                crate_versions_found: 0,
            }),
        ));
    }

    let crate_versions_found = crate_versions.len();
    let docs_storage = state.docs_storage.clone();
    state.background_tasks.spawn_once("docs storage migration", async move {
        for (crate_name, version, dir) in crate_versions {
            // Keep the folder names, the docs links in the database point to them.
            let name = DocsName::from_folder(crate_name.clone());
            match upload_dir_and_prune(&dir, "", &name, &version, &docs_storage).await {
                Ok(()) => {
                    tracing::info!(
                        "Migrated local docs for {crate_name} {version} to configured storage backend"
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to migrate local docs for {crate_name} {version} to configured storage backend: {e}"
                    );
                }
            }
        }
        tracing::info!(
            "Finished docs storage migration for {crate_versions_found} crate/version(s)"
        );
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(MigrateDocsStorageResponse {
            started: true,
            crate_versions_found,
        }),
    ))
}

/// Enumerate `{docs_path}/{crate_name}/{version}/` two levels deep. Cheap —
/// only lists directory entries, doesn't read any doc file contents.
async fn list_local_crate_versions(
    docs_path: &std::path::Path,
) -> std::io::Result<Vec<(String, Version, PathBuf)>> {
    let mut result = Vec::new();
    let mut crate_dirs = tokio::fs::read_dir(docs_path).await?;
    while let Some(crate_entry) = crate_dirs.next_entry().await? {
        if !crate_entry.file_type().await?.is_dir() {
            continue;
        }
        let crate_name = crate_entry.file_name().to_string_lossy().into_owned();

        let mut version_dirs = tokio::fs::read_dir(crate_entry.path()).await?;
        while let Some(version_entry) = version_dirs.next_entry().await? {
            if !version_entry.file_type().await?.is_dir() {
                continue;
            }
            let folder = version_entry.file_name().to_string_lossy().into_owned();
            let Ok(version) = Version::try_from(folder.as_str()) else {
                warn!("Skipping local docs of {crate_name} {folder}, the version is invalid");
                continue;
            };
            result.push((crate_name.clone(), version, version_entry.path()));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::Router;
    use axum::body::Body;
    use axum::http::Response;
    use axum::routing::{get, post};
    use bytes::Bytes;
    use http_body_util::BodyExt;
    use hyper::{Request, header};
    use kellnr_common::account_name::UserName;
    use kellnr_common::background_tasks::BackgroundTasks;
    use kellnr_common::crate_data::{CrateRegistryDep, CrateVersionData};
    use kellnr_common::crate_dependents::CrateDependent;
    use kellnr_common::publish_metadata::{PublishMetadata, RegistryDep};
    use kellnr_db::UserFlags;
    use kellnr_db::test_utils::{
        TestCrateBuilder, default_created, test_add_cached_crate_with_downloads, test_drop_table,
    };
    use kellnr_http::state::AppStateData;
    use kellnr_http::test_support::{
        TestServer, session_cookie, session_identity, sqlite_test_state, test_state,
    };
    use kellnr_settings::{Postgresql, Settings, Setup};
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::docs_storage::DocsStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
    use kellnr_storage::object_storage::MemoryStorage;
    use kellnr_storage::storage_error::StorageError;
    use kellnr_storage::storage_key::StorageKey;
    use kellnr_test_db::TestDb;
    use serde::de::DeserializeOwned;
    use tower::ServiceExt;

    use super::*;

    fn app(state: AppStateData) -> Router {
        Router::new()
            .route("/search", get(search))
            .route("/crates", get(crates))
            .route("/crate_data", get(crate_data))
            .route("/crate_dependents", get(crate_dependents))
            .route("/version", get(kellnr_version))
            .route("/statistic", get(statistic))
            .route("/build", post(build_rustdoc))
            .route("/cratesio_data", get(cratesio_data))
            .route("/settings", get(settings))
            .route("/docs_enabled", get(docs_enabled))
            .route("/sponsor_links_enabled", get(sponsor_links_enabled))
            .route("/migrate-storage", post(migrate_docs_storage))
            .with_state(state)
    }

    /// `state` with `settings` and an empty crate storage that uses them.
    fn with_settings(state: AppStateData, settings: Settings) -> AppStateData {
        let crate_storage = KellnrCrateStorage::new(&settings, Box::new(MemoryStorage::new()));
        AppStateData {
            settings: Arc::new(settings),
            crate_storage: Arc::new(crate_storage),
            ..state
        }
    }

    /// Sends a GET request, with a session if `cookie` is not empty.
    async fn get_with(state: AppStateData, uri: &str, cookie: &str) -> Response<Body> {
        send(state, Request::get(uri), cookie).await
    }

    async fn post_with(state: AppStateData, uri: &str, cookie: &str) -> Response<Body> {
        send(state, Request::post(uri), cookie).await
    }

    async fn send(
        state: AppStateData,
        request: hyper::http::request::Builder,
        cookie: &str,
    ) -> Response<Body> {
        let request = if cookie.is_empty() {
            request
        } else {
            request.header(header::COOKIE, cookie)
        };
        app(state)
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    async fn json<T: DeserializeOwned>(response: Response<Body>) -> T {
        assert_eq!(StatusCode::OK, response.status());
        let body = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    async fn add_crate(db: &TestDb, name: &str, version: &str) {
        TestCrateBuilder::new(db)
            .name(name)
            .version(version)
            .build()
            .await
            .unwrap();
    }

    async fn add_crate_with_downloads(db: &TestDb, name: &str, downloads: i64) {
        TestCrateBuilder::new(db)
            .name(name)
            .downloads(downloads)
            .build()
            .await
            .unwrap();
    }

    /// Publishes `pm` with `admin` as owner.
    async fn publish(db: &TestDb, pm: PublishMetadata) {
        db.add_crate(
            &pm,
            "cksum",
            1024,
            &default_created(),
            &UserName::new("admin").unwrap(),
            None,
        )
        .await
        .unwrap();
    }

    async fn add_user(db: &TestDb, name: &str) {
        db.add_user(&UserName::new(name).unwrap(), "pwd", UserFlags::normal())
            .await
            .unwrap();
    }

    /// Adds the download-restricted crate `name`, owned by `admin`.
    async fn add_restricted_crate(db: &TestDb, name: &str) {
        add_crate(db, name, "1.0.0").await;
        db.change_download_restricted(&canonical(name), true)
            .await
            .unwrap();
    }

    async fn add_crate_user(db: &TestDb, name: &str, user: &str) {
        db.add_crate_user(&canonical(name), &UserName::new(user).unwrap())
            .await
            .unwrap();
    }

    fn canonical(name: &str) -> CanonicalName {
        CanonicalName::try_from(name).unwrap()
    }

    async fn versions(db: &TestDb, name: &str) -> Vec<String> {
        db.get_crate_versions(&canonical(name))
            .await
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn names(crates: &[CrateOverview]) -> Vec<&str> {
        crates.iter().map(|c| c.name.as_str()).collect()
    }

    #[tokio::test]
    async fn deleting_a_version_forgets_its_known_license() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = delete_state(data_dir.path()).await;
        add_crate(&db, "licensed", "1.0.0").await;
        let name = OriginalName::try_from("licensed").unwrap();
        let version = Version::try_from("1.0.0").unwrap();
        state
            .crate_storage
            .put(&name, &version, Bytes::from_static(b"crate"))
            .await
            .unwrap();
        let licenses = state.download_filters.registry.licenses();
        licenses.insert(
            &name.to_canonical(),
            &version,
            kellnr_filter::KnownLicense::Declared("MIT".to_string()),
        );

        delete_crate_versions_impl(&state, &name, Some(vec![version.clone()]))
            .await
            .unwrap();

        // A version published again with another license is looked up anew
        assert_eq!(None, licenses.get(&name.to_canonical(), &version));
    }

    async fn delete_state(data_dir: &std::path::Path) -> (AppStateData, TestDb) {
        let (state, db) = sqlite_test_state().await;
        let settings = kellnr_settings::test_settings_in(data_dir);
        let storage = Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
        let docs_storage = Arc::new(DocsStorage::new(Box::new(
            FSStorage::new(&settings.docs_path()).unwrap(),
        ) as DynStorage));
        let state = AppStateData {
            crate_storage: Arc::new(KellnrCrateStorage::new(&settings, storage)),
            settings: Arc::new(settings),
            docs_storage,
            ..state
        };
        (state, db)
    }

    /// Storage that records deleted keys. Every key exists, deleting
    /// `fail_key` fails.
    struct RecordingStorage {
        deleted: Arc<std::sync::Mutex<Vec<String>>>,
        fail_key: Option<&'static str>,
    }

    #[async_trait::async_trait]
    impl kellnr_storage::storage::Storage for RecordingStorage {
        async fn get(&self, _key: &StorageKey) -> Result<Bytes, StorageError> {
            unimplemented!()
        }
        async fn get_with_meta(
            &self,
            _key: &StorageKey,
        ) -> Result<kellnr_storage::storage::ObjectWithMeta, StorageError> {
            unimplemented!()
        }
        async fn put(&self, _key: &StorageKey, _object: Bytes) -> Result<(), StorageError> {
            unimplemented!()
        }
        async fn put_overwrite(
            &self,
            _key: &StorageKey,
            _object: Bytes,
        ) -> Result<(), StorageError> {
            unimplemented!()
        }
        async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
            if self.fail_key == Some(key.to_string().as_str()) {
                return Err(StorageError::GenericError("delete failed".to_string()));
            }
            self.deleted.lock().unwrap().push(key.to_string());
            Ok(())
        }
        async fn exists(&self, _key: &StorageKey) -> Result<bool, StorageError> {
            Ok(true)
        }
        async fn size(&self, _key: &StorageKey) -> Result<u64, StorageError> {
            unimplemented!()
        }
        async fn list(&self, _prefix: &str) -> Result<Vec<StorageKey>, StorageError> {
            Ok(vec![])
        }
    }

    fn with_recording_storage(
        mut state: AppStateData,
        fail_key: Option<&'static str>,
    ) -> (AppStateData, Arc<std::sync::Mutex<Vec<String>>>) {
        let deleted = Arc::new(std::sync::Mutex::new(Vec::new()));
        let storage = Box::new(RecordingStorage {
            deleted: deleted.clone(),
            fail_key,
        }) as DynStorage;
        state.crate_storage = Arc::new(KellnrCrateStorage::new(&state.settings, storage));
        (state, deleted)
    }

    /// Crate files are stored under the name the crate was first published
    /// with, not under the spelling used in the delete request.
    #[tokio::test]
    async fn deleting_a_crate_removes_the_file_of_its_original_name() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = delete_state(data_dir.path()).await;
        add_crate(&db, "My_Crate", "1.0.0").await;
        let (state, deleted) = with_recording_storage(state, None);

        let requested = OriginalName::try_from("my_crate").unwrap();
        delete_crate_versions_impl(&state, &requested, None)
            .await
            .unwrap();

        assert_eq!(*deleted.lock().unwrap(), vec!["My_Crate-1.0.0.crate"]);
        assert!(versions(&db, "my_crate").await.is_empty());
    }

    /// If the crate file cannot be deleted, the database entry must stay so
    /// that the crate is not left without an entry that refers to its file.
    #[tokio::test]
    async fn failed_storage_deletion_keeps_the_database_entry() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = delete_state(data_dir.path()).await;
        add_crate(&db, "foo", "1.0.0").await;
        let (state, _) = with_recording_storage(state, Some("foo-1.0.0.crate"));

        let result = delete_crate_versions_impl(
            &state,
            &OriginalName::try_from("foo").unwrap(),
            Some(vec![Version::try_from("1.0.0").unwrap()]),
        )
        .await;

        assert!(result.is_err());
        assert_eq!(versions(&db, "foo").await, ["1.0.0"]);
    }

    #[tokio::test]
    async fn deleting_an_unknown_crate_is_not_found() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, _db) = delete_state(data_dir.path()).await;

        let result =
            delete_crate_versions_impl(&state, &OriginalName::try_from("nope").unwrap(), None)
                .await;

        assert!(
            result
                .as_ref()
                .is_err_and(|e| e.status() == StatusCode::NOT_FOUND)
        );
    }

    /// A failing version must not keep the remaining versions from being
    /// deleted.
    #[tokio::test]
    async fn deleting_all_versions_continues_after_a_failure() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = delete_state(data_dir.path()).await;
        add_crate(&db, "foo", "1.0.0").await;
        add_crate(&db, "foo", "2.0.0").await;
        let (state, deleted) = with_recording_storage(state, Some("foo-1.0.0.crate"));

        let result =
            delete_crate_versions_impl(&state, &OriginalName::try_from("foo").unwrap(), None).await;

        assert!(result.is_err());
        assert_eq!(*deleted.lock().unwrap(), vec!["foo-2.0.0.crate"]);
        assert_eq!(versions(&db, "foo").await, ["1.0.0"]);
    }

    #[tokio::test]
    async fn settings_returns_from_settings() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let response: SettingsResponse = json(get_with(state, "/settings", &cookie).await).await;

        // Set the password to empty string because it is not serialized
        // and expect the configured admin password to be redacted.
        let tmp = kellnr_settings::test_settings();
        let psq = Postgresql {
            pwd: String::default(),
            ..tmp.postgresql
        };
        let setup = Setup {
            admin_pwd: REDACTED.to_string(),
            ..tmp.setup
        };
        let expected_state = Settings {
            postgresql: psq,
            setup,
            ..tmp
        };
        let result_settings: Settings =
            serde_json::from_value(serde_json::Value::Object(response.settings)).unwrap();

        assert_eq!(result_settings, expected_state);
        // `sources` is structurally part of `SettingsResponse`, successful
        // deserialization above is sufficient proof it round-trips.
        // The companion test below asserts that an override actually flows
        // into the `sources` map.
    }

    #[tokio::test]
    async fn settings_response_sources_reflect_prov_overrides() {
        use kellnr_settings::{Config, ConfigSource, SettingsProv};

        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        // Build a real `SettingsProv` with one non-default leaf so the
        // handler must emit a non-empty `sources` entry for it. Using only
        // TOML keeps the test independent of any `KELLNR_*` process env.
        let prov: SettingsProv = Config::new()
            .add_toml_str(
                "override.toml",
                "[registry]\ndata_dir = \"/tmp/from-toml\"\n",
            )
            .build::<SettingsProv>()
            .unwrap();
        let state = AppStateData {
            settings_prov: Arc::new(prov),
            ..state
        };

        let response: SettingsResponse = json(get_with(state, "/settings", &cookie).await).await;

        assert_eq!(
            response.sources.get("registry.data_dir"),
            Some(&ConfigSource::Toml),
            "TOML-set leaf must report ConfigSource::Toml"
        );
        // An untouched leaf must surface as Default, confirms the map covers
        // every leaf and that File→Toml mapping doesn't bleed across keys.
        assert_eq!(
            response.sources.get("docs.enabled"),
            Some(&ConfigSource::Default),
        );

        // The new `leaves` field must mirror the same data with the schema
        // metadata the dynamic UI consumes.
        let data_dir_leaf = response
            .leaves
            .iter()
            .find(|l| l.key == "registry.data_dir")
            .expect("registry.data_dir leaf must be present");
        assert_eq!(
            data_dir_leaf.value,
            serde_json::json!("/tmp/from-toml"),
            "leaf value matches the TOML override"
        );
        assert_eq!(data_dir_leaf.source, ConfigSource::Toml);
        assert_eq!(data_dir_leaf.kind, LeafKind::String);
        assert!(!data_dir_leaf.secret);
        assert_eq!(
            data_dir_leaf.cli_flag.as_deref(),
            Some("--registry-data-dir, -d"),
            "cli_flag comes from clap reflection, long + short"
        );

        let docs_enabled = response
            .leaves
            .iter()
            .find(|l| l.key == "docs.enabled")
            .expect("docs.enabled leaf must be present");
        assert_eq!(docs_enabled.kind, LeafKind::Boolean);
        assert_eq!(docs_enabled.source, ConfigSource::Default);
        assert_eq!(docs_enabled.default, serde_json::json!(false));

        // Secret leaves carry the secret flag, the UI uses it to mask the
        // value before rendering.
        let secret_leaf = response
            .leaves
            .iter()
            .find(|l| l.key == "s3.secret_key")
            .expect("s3.secret_key leaf must be present");
        assert!(secret_leaf.secret, "s3.secret_key must be flagged secret");
    }

    #[tokio::test]
    async fn settings_response_redacts_secrets() {
        use kellnr_settings::{Config, SettingsProv};

        const SECRETS: [&str; 3] = ["s3-secret-value", "admin-pwd-value", "admin-token-value"];
        let toml = format!(
            "[s3]\nsecret_key = \"{}\"\n[setup]\nadmin_pwd = \"{}\"\nadmin_token = \"{}\"\n",
            SECRETS[0], SECRETS[1], SECRETS[2]
        );
        let prov: SettingsProv = Config::new()
            .add_toml_str("secrets.toml", &toml)
            .build::<SettingsProv>()
            .unwrap();
        let mut settings = kellnr_settings::test_settings();
        settings.s3.secret_key = Some(SECRETS[0].to_string());
        settings.setup.admin_pwd = SECRETS[1].to_string();
        settings.setup.admin_token = Some(SECRETS[2].to_string());

        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        let state = AppStateData {
            settings_prov: Arc::new(prov),
            ..with_settings(state, settings)
        };

        let r = get_with(state, "/settings", &cookie).await;

        assert_eq!(r.status(), StatusCode::OK);
        let body = r.into_body().collect().await.unwrap().to_bytes();
        let raw = String::from_utf8(body.to_vec()).unwrap();
        for secret in SECRETS {
            assert!(!raw.contains(secret), "response leaks secret {secret}");
        }
        // The default admin password must not leak via `defaults` either.
        assert!(!raw.contains("\"admin_pwd\":\"admin\""));

        let response: SettingsResponse = serde_json::from_slice(&body).unwrap();
        let redacted = serde_json::json!(REDACTED);
        assert_eq!(response.settings["s3"]["secret_key"], redacted);
        assert_eq!(response.settings["setup"]["admin_pwd"], redacted);
        assert_eq!(response.settings["setup"]["admin_token"], redacted);
        assert_eq!(response.defaults["setup"]["admin_pwd"], redacted);
        assert_eq!(
            response.defaults["setup"]["admin_token"],
            serde_json::Value::Null
        );

        for leaf in response.leaves.iter().filter(|leaf| leaf.secret) {
            assert_eq!(leaf.kind, LeafKind::String, "{}", leaf.key);
            assert!(
                leaf.value.is_null() || leaf.value == redacted,
                "secret leaf {} is not redacted",
                leaf.key
            );
        }
        let unset_secret = response
            .leaves
            .iter()
            .find(|l| l.key == "oauth2.client_secret")
            .expect("oauth2.client_secret leaf must be present");
        assert!(unset_secret.value.is_null(), "unset secret must be null");
    }

    #[tokio::test]
    async fn docs_enabled_returns_false_by_default() {
        let response: DocsEnabledResponse =
            json(get_with(test_state(), "/docs_enabled", "").await).await;

        assert!(!response.enabled);
    }

    #[tokio::test]
    async fn docs_enabled_returns_true_when_enabled() {
        let mut settings = kellnr_settings::test_settings();
        settings.docs.enabled = true;
        let state = with_settings(test_state(), settings);

        let response: DocsEnabledResponse = json(get_with(state, "/docs_enabled", "").await).await;

        assert!(response.enabled);
    }

    #[tokio::test]
    async fn sponsor_links_enabled_returns_true_by_default() {
        let response: SponsorLinksEnabledResponse =
            json(get_with(test_state(), "/sponsor_links_enabled", "").await).await;

        assert!(response.enabled);
    }

    #[tokio::test]
    async fn sponsor_links_enabled_returns_false_when_disabled() {
        let mut settings = kellnr_settings::test_settings();
        settings.registry.sponsor_links_enabled = false;
        let state = with_settings(test_state(), settings);

        let response: SponsorLinksEnabledResponse =
            json(get_with(state, "/sponsor_links_enabled", "").await).await;

        assert!(!response.enabled);
    }

    /// State with docs enabled. Queueing a build writes the crate to the doc
    /// queue directory in `data_dir`.
    async fn docs_state(data_dir: &std::path::Path) -> (AppStateData, TestDb) {
        let (state, db) = sqlite_test_state().await;
        let mut settings = kellnr_settings::test_settings_in(data_dir);
        settings.docs.enabled = true;
        (with_settings(state, settings), db)
    }

    async fn build_foobar_docs(state: AppStateData, cookie: &str) -> StatusCode {
        post_with(state, "/build?package=foobar&version=1.0.0", cookie)
            .await
            .status()
    }

    async fn doc_queue_len(db: &TestDb) -> usize {
        db.get_doc_queue().await.unwrap().len()
    }

    #[tokio::test]
    async fn build_rust_doc_crate_not_found() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = docs_state(data_dir.path()).await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let status = build_foobar_docs(state, &cookie).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // A non-owner gets the same response for a missing crate as for a crate
    // of someone else, so that the existence of a crate is not revealed.
    #[tokio::test]
    async fn build_rust_doc_crate_not_found_for_non_owner() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = docs_state(data_dir.path()).await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;

        let status = build_foobar_docs(state, &cookie).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn build_rust_doc_version_not_found() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = docs_state(data_dir.path()).await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        TestCrateBuilder::new(&db)
            .name("foobar")
            .owner("user")
            .version("2.0.0")
            .build()
            .await
            .unwrap();

        let status = build_foobar_docs(state, &cookie).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn build_rust_doc_not_owner() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = docs_state(data_dir.path()).await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        add_crate(&db, "foobar", "1.0.0").await;

        let status = build_foobar_docs(state, &cookie).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(doc_queue_len(&db).await, 0);
    }

    #[tokio::test]
    async fn build_rust_doc_is_owner() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = docs_state(data_dir.path()).await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        TestCrateBuilder::new(&db)
            .name("foobar")
            .owner("user")
            .build()
            .await
            .unwrap();

        let status = build_foobar_docs(state, &cookie).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(doc_queue_len(&db).await, 1);
    }

    #[tokio::test]
    async fn build_rust_doc_read_only_owner_is_rejected() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = docs_state(data_dir.path()).await;
        let cookie = session_cookie(&db, "user", UserFlags::read_only()).await;
        TestCrateBuilder::new(&db)
            .name("foobar")
            .owner("user")
            .build()
            .await
            .unwrap();

        let status = build_foobar_docs(state, &cookie).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(doc_queue_len(&db).await, 0);
    }

    #[tokio::test]
    async fn build_rust_doc_not_owner_but_admin() {
        let data_dir = tempfile::tempdir().unwrap();
        let (state, db) = docs_state(data_dir.path()).await;
        let cookie = session_cookie(&db, "other_admin", UserFlags::admin()).await;
        add_crate(&db, "foobar", "1.0.0").await;

        let status = build_foobar_docs(state, &cookie).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(doc_queue_len(&db).await, 1);
    }

    fn fs_docs_storage(dir: &std::path::Path) -> DocsStorage {
        DocsStorage::new(Box::new(FSStorage::new(dir.to_str().unwrap()).unwrap()) as DynStorage)
    }

    /// Settings with a bucket configured, i.e. `docs_backend_is_local() == false`.
    /// Only settings-level config is exercised here (no real S3 network access):
    /// the endpoint's actual copy destination is whatever `docs_storage` the
    /// caller wires into `AppStateData`, independent of this bucket name.
    fn settings_with_docs_bucket_configured(data_dir: &std::path::Path) -> Settings {
        let mut settings = kellnr_settings::test_settings();
        settings.registry.data_dir = data_dir.to_str().unwrap().to_string();
        settings.s3.enabled = true;
        settings.s3.docs_bucket = Some("kellnr-docs".to_string());
        settings
    }

    /// Posts to `/migrate-storage` as `user`, with docs migrated to `target`.
    async fn migrate_storage(
        settings: Settings,
        target: &std::path::Path,
        background_tasks: BackgroundTasks,
        user: (&str, UserFlags),
    ) -> Response<Body> {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, user.0, user.1).await;
        let state = AppStateData {
            docs_storage: Arc::new(fs_docs_storage(target)),
            background_tasks,
            ..with_settings(state, settings)
        };
        post_with(state, "/migrate-storage", &cookie).await
    }

    const ADMIN: (&str, UserFlags) = ("admin", UserFlags::admin());

    #[tokio::test]
    async fn migrate_docs_storage_returns_403_for_non_admin() {
        let local = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let settings = settings_with_docs_bucket_configured(local.path());

        let r = migrate_storage(
            settings,
            target.path(),
            BackgroundTasks::new(),
            ("user", UserFlags::normal()),
        )
        .await;

        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn migrate_docs_storage_returns_400_when_backend_is_local_disk() {
        let local = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let mut settings = kellnr_settings::test_settings();
        settings.registry.data_dir = local.path().to_str().unwrap().to_string();

        let r = migrate_storage(settings, target.path(), BackgroundTasks::new(), ADMIN).await;

        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn migrate_docs_storage_returns_zero_count_when_local_dir_missing() {
        let local = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        // `local` exists but nothing was ever generated locally, so
        // `{data_dir}/docs` itself was never created.
        let settings = settings_with_docs_bucket_configured(local.path());

        let r = migrate_storage(settings, target.path(), BackgroundTasks::new(), ADMIN).await;

        let response: MigrateDocsStorageResponse = json(r).await;
        assert!(!response.started);
        assert_eq!(response.crate_versions_found, 0);
    }

    #[tokio::test]
    async fn migrate_docs_storage_copies_local_docs_into_configured_backend() {
        let local = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let settings = settings_with_docs_bucket_configured(local.path());

        // Both auto-generated and manually-published docs land in this exact
        // on-disk layout (see `upload_dir_and_prune`), so one local crate/
        // version dir stands in for either origin.
        let docs_path_string = settings.docs_path();
        let docs_path = std::path::Path::new(&docs_path_string);
        tokio::fs::create_dir_all(docs_path.join("crate-a/1.0.0/doc"))
            .await
            .unwrap();
        tokio::fs::write(
            docs_path.join("crate-a/1.0.0/doc/index.html"),
            b"crate-a docs",
        )
        .await
        .unwrap();
        tokio::fs::create_dir_all(docs_path.join("crate-b/2.0.0/doc/nested"))
            .await
            .unwrap();
        tokio::fs::write(
            docs_path.join("crate-b/2.0.0/doc/nested/page.html"),
            b"crate-b nested docs",
        )
        .await
        .unwrap();

        let tasks = BackgroundTasks::new();
        let r = migrate_storage(settings, target.path(), tasks.clone(), ADMIN).await;

        assert_eq!(r.status(), StatusCode::ACCEPTED);
        let body = r.into_body().collect().await.unwrap().to_bytes();
        let response: MigrateDocsStorageResponse = serde_json::from_slice(&body).unwrap();
        assert!(response.started);
        assert_eq!(response.crate_versions_found, 2);

        let key_a = DocsStorage::file_key(
            &DocsName::from_folder("crate-a".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            "doc/index.html",
        );
        let key_b = DocsStorage::file_key(
            &DocsName::from_folder("crate-b".to_string()),
            &Version::try_from("2.0.0").unwrap(),
            "doc/nested/page.html",
        );
        // Shutdown waits for the migration to finish.
        tasks.request_shutdown();
        tasks.wait_for_shutdown(Duration::from_secs(30)).await;

        // A second storage on the same directory sees what the migration wrote.
        let target_docs_storage = fs_docs_storage(target.path());
        assert_eq!(
            target_docs_storage.get(&key_a).await.unwrap(),
            Bytes::from_static(b"crate-a docs")
        );
        assert_eq!(
            target_docs_storage.get(&key_b).await.unwrap(),
            Bytes::from_static(b"crate-b nested docs")
        );
    }

    /// A failing database must not look like an empty registry.
    #[tokio::test]
    async fn statistic_database_error_is_internal_error() {
        let (state, db) = sqlite_test_state().await;
        test_drop_table(&db, "krate").await.unwrap();

        let r = get_with(state, "/statistic", "").await;

        assert_eq!(r.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    fn top(name: &str, downloads: u64) -> (String, u64) {
        (name.to_string(), downloads)
    }

    fn last_updated(name: &str) -> (OriginalName, Version) {
        (
            OriginalName::from_unchecked(name.to_string()),
            Version::try_from("1.0.0").unwrap(),
        )
    }

    #[tokio::test]
    async fn statistic_returns_sparse_statistics() {
        let (state, db) = sqlite_test_state().await;
        add_crate_with_downloads(&db, "top1", 1000).await;

        let result: Statistic = json(get_with(state, "/statistic", "").await).await;

        let expect = Statistic {
            num_crates: 1,
            num_crate_versions: 1,
            num_crate_downloads: 1000,
            num_proxy_crates: 0,
            num_proxy_crate_versions: 0,
            num_proxy_crate_downloads: 0,
            top_crates: TopCrates {
                first: Some(top("top1", 1000)),
                second: None,
                third: None,
            },
            last_updated_crate: Some(last_updated("top1")),
            proxy_enabled: false,
        };
        assert_eq!(expect, result);
    }

    #[tokio::test]
    async fn statistic_returns_empty_statistics() {
        let (state, _db) = sqlite_test_state().await;

        let result: Statistic = json(get_with(state, "/statistic", "").await).await;

        let expect = Statistic {
            num_crates: 0,
            num_crate_versions: 0,
            num_crate_downloads: 0,
            num_proxy_crates: 0,
            num_proxy_crate_versions: 0,
            num_proxy_crate_downloads: 0,
            top_crates: TopCrates {
                first: None,
                second: None,
                third: None,
            },
            last_updated_crate: None,
            proxy_enabled: false,
        };
        assert_eq!(expect, result);
    }

    #[tokio::test]
    async fn statistic_returns_crate_statistics() {
        let (state, db) = sqlite_test_state().await;
        add_crate_with_downloads(&db, "top1", 1000).await;
        add_crate_with_downloads(&db, "top2", 500).await;
        add_crate_with_downloads(&db, "top3", 100).await;
        TestCrateBuilder::new(&db)
            .name("foobar")
            .downloads(10)
            .created(default_created() + Duration::from_secs(60))
            .build()
            .await
            .unwrap();
        test_add_cached_crate_with_downloads(&db, "cached1", "1.0.0", 9)
            .await
            .unwrap();
        test_add_cached_crate_with_downloads(&db, "cached2", "1.0.0", 90)
            .await
            .unwrap();

        let result: Statistic = json(get_with(state, "/statistic", "").await).await;

        let expect = Statistic {
            num_crates: 4,
            num_crate_versions: 4,
            num_crate_downloads: 1610,
            num_proxy_crates: 2,
            num_proxy_crate_versions: 2,
            num_proxy_crate_downloads: 99,
            top_crates: TopCrates {
                first: Some(top("top1", 1000)),
                second: Some(top("top2", 500)),
                third: Some(top("top3", 100)),
            },
            last_updated_crate: Some(last_updated("foobar")),
            proxy_enabled: false,
        };
        assert_eq!(expect, result);
    }

    #[tokio::test]
    async fn statistic_user_session_hides_inaccessible_crate_names() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        add_restricted_crate(&db, "secret").await;

        let result: Statistic = json(get_with(state, "/statistic", &cookie).await).await;

        assert_eq!(1, result.num_crates);
        assert_eq!(None, result.top_crates.first);
        assert_eq!(None, result.last_updated_crate);
    }

    #[tokio::test]
    async fn kellnr_version_returns_version() {
        let result: KellnrVersion = json(get_with(test_state(), "/version", "").await).await;

        assert_eq!("0.0.0-unknown", result.version);
    }

    #[tokio::test]
    async fn search_not_hits_returns_nothing() {
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "hello", "1.0.0").await;

        let result: Pagination = json(get_with(state, "/search?name=doesnotexist", "").await).await;

        assert_eq!(0, result.crates.len());
        assert_eq!(0, result.page);
        assert_eq!(10, result.page_size);
    }

    // The length bound lives on `NameOrDescription`. It has to hold here too,
    // where the value arrives through serde rather than an explicit `TryFrom`.
    #[tokio::test]
    async fn search_rejects_overlong_query() {
        let uri = format!("/search?name={}", "a".repeat(65));

        let r = get_with(test_state(), &uri, "").await;

        assert_eq!(StatusCode::BAD_REQUEST, r.status());
    }

    #[tokio::test]
    async fn search_accepts_multi_word_query() {
        let (state, _db) = sqlite_test_state().await;

        let r = get_with(state, "/search?name=vcard%20parser", "").await;

        assert_eq!(StatusCode::OK, r.status());
    }

    #[tokio::test]
    async fn search_returns_only_searched_results() {
        let (state, db) = sqlite_test_state().await;
        add_crate_with_downloads(&db, "hello", 2).await;
        add_crate(&db, "other", "1.0.0").await;

        let result: Pagination = json(get_with(state, "/search?name=hello", "").await).await;

        assert_eq!(names(&result.crates), ["hello"]);
        assert_eq!("1.0.0", result.crates[0].version);
        assert_eq!(2, result.crates[0].total_downloads);
        assert_eq!(0, result.page);
        assert_eq!(10, result.page_size);
    }

    async fn add_hello_crates(db: &TestDb, count: usize) {
        for i in 0..count {
            add_crate(db, &format!("hello{i}"), "1.0.0").await;
        }
    }

    // Search has to page through the database the same way the crate overview
    // does, instead of asking for a single fixed-size window. A page beyond the
    // first is only reachable if `page` is translated into an offset.
    #[tokio::test]
    async fn search_paginates_like_crate_overview() {
        let (state, db) = sqlite_test_state().await;
        add_hello_crates(&db, 5).await;

        let result: Pagination =
            json(get_with(state, "/search?name=hello&page=1&page_size=2", "").await).await;

        assert_eq!(names(&result.crates), ["hello2", "hello3"]);
        assert_eq!(1, result.page);
        assert_eq!(2, result.page_size);
    }

    // The page size is whatever the caller asked for, so a full page cannot be
    // mistaken for the end of the results.
    #[tokio::test]
    async fn search_reports_requested_page_size_on_a_full_page() {
        let (state, db) = sqlite_test_state().await;
        add_hello_crates(&db, 3).await;

        let result: Pagination =
            json(get_with(state, "/search?name=hello&page_size=2", "").await).await;

        assert_eq!(2, result.crates.len());
        assert_eq!(2, result.page_size);
    }

    #[tokio::test]
    async fn search_admin_session_lists_all_crates() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;
        add_crate(&db, "foo", "1.0.0").await;
        add_restricted_crate(&db, "foo-secret").await;

        let result: Pagination = json(get_with(state, "/search?name=foo", &cookie).await).await;

        assert_eq!(names(&result.crates), ["foo", "foo-secret"]);
    }

    #[tokio::test]
    async fn crate_get_crate_information() {
        let (state, db) = sqlite_test_state().await;
        publish(
            &db,
            PublishMetadata {
                name: "crate1".to_string(),
                vers: "1.0.0".to_string(),
                description: Some("description".to_string()),
                homepage: Some("homepage".to_string()),
                readme: Some("readme".to_string()),
                license: Some("MIT".to_string()),
                keywords: vec!["key1".to_string()],
                categories: vec!["cat1".to_string()],
                authors: Some(vec!["author1".to_string()]),
                ..PublishMetadata::default()
            },
        )
        .await;

        let result: CrateData =
            json(get_with(state, "/crate_data?name=crate1&version=1.0.0", "").await).await;

        assert_eq!(
            db.get_crate_data(&canonical("crate1")).await.unwrap(),
            result
        );
        assert_eq!("crate1", result.name);
        assert_eq!(Some("description".to_string()), result.description);
        assert_eq!(Some("readme".to_string()), result.versions[0].readme);
    }

    #[tokio::test]
    async fn crate_data_hides_dependency_info_of_restricted_crates() {
        let db = TestDb::sqlite().await;
        add_restricted_crate(&db, "secret").await;
        add_crate(&db, "public", "1.0.0").await;

        let dep = |name: &str, registry: Option<&str>| CrateRegistryDep {
            name: name.to_string(),
            description: Some("description".to_string()),
            version_req: "^1".to_string(),
            registry: registry.map(str::to_string),
            latest_version: Some("2.0.0".to_string()),
            outdated: true,
            ..CrateRegistryDep::default()
        };
        let mut cd = CrateData {
            versions: vec![CrateVersionData {
                dependencies: vec![
                    dep("secret", None),
                    dep("public", None),
                    dep("serde", Some(CRATESIO_INDEX)),
                ],
                ..CrateVersionData::default()
            }],
            ..CrateData::default()
        };

        hide_restricted_dep_info(&mut cd, None, &db.provider())
            .await
            .unwrap();

        let deps = &cd.versions[0].dependencies;
        assert_eq!(None, deps[0].description);
        assert_eq!(None, deps[0].latest_version);
        assert!(!deps[0].outdated);
        assert_eq!(dep("public", None), deps[1]);
        assert_eq!(dep("serde", Some(CRATESIO_INDEX)), deps[2]);
    }

    fn dependent(name: &str) -> CrateDependent {
        CrateDependent {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            outdated: true,
            ..CrateDependent::default()
        }
    }

    #[tokio::test]
    async fn crate_dependents_counts_restricted_dependents_only() {
        let db = TestDb::sqlite().await;
        add_restricted_crate(&db, "secret").await;
        let mut cd = CrateDependents {
            latest_version: Some("2.0.0".to_string()),
            dependents: vec![dependent("Secret"), dependent("public")],
            restricted: 0,
        };

        hide_restricted_dependents(&mut cd, None, &db.provider())
            .await
            .unwrap();

        assert_eq!(vec![dependent("public")], cd.dependents);
        assert_eq!(1, cd.restricted);
    }

    #[tokio::test]
    async fn crate_dependents_shows_restricted_dependents_with_access() {
        let db = TestDb::sqlite().await;
        add_user(&db, "user").await;
        add_restricted_crate(&db, "secret").await;
        add_crate_user(&db, "secret", "user").await;
        let user = session_identity("user", false);
        let mut cd = CrateDependents {
            dependents: vec![dependent("secret")],
            ..CrateDependents::default()
        };

        hide_restricted_dependents(&mut cd, Some(&user), &db.provider())
            .await
            .unwrap();

        assert_eq!(vec![dependent("secret")], cd.dependents);
        assert_eq!(0, cd.restricted);
    }

    /// Publishes `name` 1.0.0 with a dependency on `foo`.
    async fn publish_foo_dependent(db: &TestDb, name: &str) {
        publish(
            db,
            PublishMetadata {
                name: name.to_string(),
                vers: "1.0.0".to_string(),
                deps: vec![RegistryDep {
                    name: "foo".to_string(),
                    version_req: "^1".to_string(),
                    features: None,
                    optional: false,
                    default_features: true,
                    target: None,
                    kind: Some("normal".to_string()),
                    registry: None,
                    explicit_name_in_toml: None,
                }],
                ..PublishMetadata::default()
            },
        )
        .await;
    }

    #[tokio::test]
    async fn crate_dependents_returns_dependents() {
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "foo", "1.0.0").await;
        publish_foo_dependent(&db, "public").await;
        publish_foo_dependent(&db, "secret").await;
        db.change_download_restricted(&canonical("secret"), true)
            .await
            .unwrap();

        let cd: CrateDependents =
            json(get_with(state, "/crate_dependents?name=Foo", "").await).await;

        assert_eq!(Some("1.0.0".to_string()), cd.latest_version);
        assert_eq!(
            ["public"],
            cd.dependents
                .iter()
                .map(|d| d.name.as_str())
                .collect::<Vec<_>>()[..]
        );
        assert_eq!(1, cd.restricted);
    }

    #[tokio::test]
    async fn crate_dependents_of_restricted_crate_anonymous_returns_not_found() {
        let (state, db) = sqlite_test_state().await;
        add_restricted_crate(&db, "secret").await;

        let r = get_with(state, "/crate_dependents?name=secret", "").await;

        assert_eq!(StatusCode::NOT_FOUND, r.status());
    }

    #[tokio::test]
    async fn crate_dependents_of_unknown_crate_returns_not_found() {
        let (state, _db) = sqlite_test_state().await;

        let r = get_with(state, "/crate_dependents?name=missing", "").await;

        assert_eq!(StatusCode::NOT_FOUND, r.status());
    }

    /// Status of `/crate_data` for the restricted crate `secret` as `user`,
    /// anonymous if `None`. `prepare` sets up access to the crate.
    async fn get_secret_crate_data(
        user: Option<(&str, UserFlags)>,
        prepare: impl AsyncFnOnce(&TestDb),
    ) -> StatusCode {
        let (state, db) = sqlite_test_state().await;
        add_restricted_crate(&db, "secret").await;
        let cookie = match user {
            Some((name, flags)) => session_cookie(&db, name, flags).await,
            None => String::new(),
        };
        prepare(&db).await;
        get_with(state, "/crate_data?name=secret", &cookie)
            .await
            .status()
    }

    #[tokio::test]
    async fn crate_data_restricted_crate_anonymous_returns_not_found() {
        let status = get_secret_crate_data(None, async |_| {}).await;

        assert_eq!(StatusCode::NOT_FOUND, status);
    }

    #[tokio::test]
    async fn crate_data_restricted_crate_without_access_returns_not_found() {
        let status = get_secret_crate_data(Some(("user", UserFlags::normal())), async |_| {}).await;

        assert_eq!(StatusCode::NOT_FOUND, status);
    }

    #[tokio::test]
    async fn crate_data_restricted_crate_crate_user_returns_ok() {
        let status = get_secret_crate_data(Some(("user", UserFlags::normal())), async |db| {
            add_crate_user(db, "secret", "user").await;
        })
        .await;

        assert_eq!(StatusCode::OK, status);
    }

    #[tokio::test]
    async fn crate_data_restricted_crate_admin_returns_ok() {
        let status =
            get_secret_crate_data(Some(("other_admin", UserFlags::admin())), async |_| {}).await;

        assert_eq!(StatusCode::OK, status);
    }

    #[tokio::test]
    async fn crates_user_session_lists_accessible_restricted_crates() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        add_crate(&db, "open", "1.0.0").await;
        add_restricted_crate(&db, "secret").await;
        add_crate_user(&db, "secret", "user").await;
        add_restricted_crate(&db, "hidden").await;

        let result: Pagination = json(get_with(state, "/crates", &cookie).await).await;

        assert_eq!(names(&result.crates), ["open", "secret"]);
        assert_eq!(2, result.total);
    }

    /// A failing database must not look like an empty registry.
    #[tokio::test]
    async fn crates_database_error_is_internal_error() {
        let (state, db) = sqlite_test_state().await;
        test_drop_table(&db, "krate").await.unwrap();

        let r = get_with(state, "/crates", "").await;

        assert_eq!(r.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn crates_get_page() {
        let (state, db) = sqlite_test_state().await;
        for i in 0..12 {
            add_crate(&db, &format!("c{i:02}"), "1.0.0").await;
        }

        let result: Pagination = json(get_with(state, "/crates?page=0", "").await).await;

        let expected: Vec<String> = (0..10).map(|i| format!("c{i:02}")).collect();
        assert_eq!(names(&result.crates), expected);
        assert_eq!(0, result.page);
        assert_eq!(10, result.page_size);
        assert_eq!(12, result.total);
    }

    #[tokio::test]
    async fn crates_get_all_crates() {
        let (state, db) = sqlite_test_state().await;
        add_crate(&db, "c1", "1.0.0").await;
        add_crate(&db, "c2", "2.0.0").await;
        add_crate(&db, "c3", "3.0.0").await;

        let result: Pagination = json(get_with(state, "/crates", "").await).await;

        assert_eq!(names(&result.crates), ["c1", "c2", "c3"]);
        assert_eq!(
            ["1.0.0", "2.0.0", "3.0.0"],
            result
                .crates
                .iter()
                .map(|c| c.version.as_str())
                .collect::<Vec<_>>()[..]
        );
        assert_eq!(0, result.page);
        assert_eq!(10, result.page_size);
        assert_eq!(3, result.total);
    }

    #[tokio::test]
    async fn crates_sorts_by_requested_order() {
        let (state, db) = sqlite_test_state().await;
        add_crate_with_downloads(&db, "a", 1).await;
        add_crate_with_downloads(&db, "b", 3).await;
        add_crate_with_downloads(&db, "c", 2).await;

        let result: Pagination =
            json(get_with(state, "/crates?page_size=2&sort=downloads", "").await).await;

        assert_eq!(names(&result.crates), ["b", "c"]);
    }

    /// Publishes `name` with `keywords` and `categories`.
    async fn publish_tagged(db: &TestDb, name: &str, keywords: &[&str], categories: &[&str]) {
        publish(
            db,
            PublishMetadata {
                name: name.to_string(),
                vers: "1.0.0".to_string(),
                keywords: keywords.iter().map(ToString::to_string).collect(),
                categories: categories.iter().map(ToString::to_string).collect(),
                ..PublishMetadata::default()
            },
        )
        .await;
    }

    async fn add_tagged_crates(db: &TestDb) {
        publish_tagged(db, "cli-tool", &["cli"], &["development-tools"]).await;
        publish_tagged(db, "cli-only", &["cli"], &[]).await;
        publish_tagged(db, "plain", &[], &[]).await;
    }

    #[tokio::test]
    async fn crates_filters_by_keyword_and_category() {
        let (state, db) = sqlite_test_state().await;
        add_tagged_crates(&db).await;

        let result: Pagination =
            json(get_with(state, "/crates?keyword=cli&category=development-tools", "").await).await;

        assert_eq!(names(&result.crates), ["cli-tool"]);
        assert_eq!(1, result.total);
    }

    #[tokio::test]
    async fn crates_ignores_empty_keyword_and_category() {
        let (state, db) = sqlite_test_state().await;
        add_tagged_crates(&db).await;

        let result: Pagination =
            json(get_with(state, "/crates?keyword=&category=", "").await).await;

        assert_eq!(names(&result.crates), ["cli-only", "cli-tool", "plain"]);
    }

    #[tokio::test]
    async fn crates_rejects_unknown_sort_order() {
        let r = get_with(test_state(), "/crates?sort=stars", "").await;

        assert_eq!(StatusCode::BAD_REQUEST, r.status());
    }

    #[tokio::test]
    async fn cratesio_data_returns_data() {
        let r = get_with(test_state(), "/cratesio_data?name=quote", "").await;

        let result_status = r.status();
        let body =
            String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
        assert!(body.contains("quote"));
        assert_eq!(StatusCode::OK, result_status);
    }

    #[tokio::test]
    async fn cratesio_data_not_found() {
        let r = get_with(test_state(), "/cratesio_data?name=thisdoesnotevenexist", "").await;

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    /// Errors of crates.io like rate limits are not a missing crate.
    #[tokio::test]
    async fn cratesio_data_upstream_error_is_bad_gateway() {
        let upstream =
            TestServer::start(Router::new().fallback(|| async { StatusCode::TOO_MANY_REQUESTS }))
                .await;
        let mut settings = kellnr_settings::test_settings();
        settings.proxy.api = upstream.url("/api/v1/crates/").parse().unwrap();

        let r = get_with(
            with_settings(test_state(), settings),
            "/cratesio_data?name=quote",
            "",
        )
        .await;

        assert_eq!(r.status(), StatusCode::BAD_GATEWAY);
    }
}
