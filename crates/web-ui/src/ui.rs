use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use kellnr_appstate::{AppState, DbState, SettingsProvState, SettingsState};
use kellnr_common::crate_data::CrateData;
use kellnr_common::crate_overview::CrateOverview;
use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::error::DbError;
use kellnr_settings::{
    ConfigSource, Provenance, Settings, SettingsProv, SourceMap, cli_flag_map, compile_time_config,
    erased_serde, leaf_label, sources_from_prov,
};
use serde::{Deserialize, Serialize};
use tracing::error;
use utoipa::ToSchema;

use crate::error::RouteError;
use crate::session::{AdminUser, MaybeUser};

/// Settings response — the flattened `Settings`, the per-leaf source map
/// derived from the `SettingsProv`, and the compiled-in defaults the UI
/// uses to show "default: X" next to overridden values.
///
/// `leaves` is the new dynamic schema: one entry per `SettingsProv` leaf with
/// the value, default, source, type, secret flag, optional CLI flag string
/// (auto-derived from clap) and optional label override. The Vue UI iterates
/// over this list instead of hand-maintaining a parallel per-leaf table.
#[derive(Serialize, Deserialize)]
pub struct SettingsResponse {
    #[serde(flatten)]
    pub settings: Settings,
    pub sources: SourceMap,
    pub defaults: Settings,
    pub leaves: Vec<LeafMeta>,
}

/// Per-leaf metadata for the dynamic UI. All fields except `value`/`source`
/// are stable per build (or, in the case of `default`, per binary), so the UI
/// can treat the response as both data and schema.
#[derive(Serialize, Deserialize, Clone, Debug)]
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
    /// `true` for `#[configurable(secret)]` leaves — the UI masks the value.
    pub secret: bool,
    /// CLI flag string (e.g. `"--registry-data-dir, -d"`) when one exists.
    pub cli_flag: Option<String>,
    /// Optional display label override; `None` means the UI humanizes the key.
    pub label: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
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
        let leaf_default = lookup_default(&defaults_json, path);
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
    // prefix anyway, so any consistent order is fine — alphabetic keeps the
    // response diff-friendly across runs.
    leaves.sort_by(|a, b| a.key.cmp(&b.key));
    leaves
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
        (status = 200, description = "Kellnr settings with source tracking"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
#[allow(clippy::unused_async)] // part of the router
pub async fn settings(
    _user: AdminUser,
    State(settings): SettingsState,
    State(prov): SettingsProvState,
) -> Result<Json<SettingsResponse>, RouteError> {
    Ok(Json(SettingsResponse {
        sources: sources_from_prov(&prov),
        leaves: build_leaves(&prov),
        settings: (*settings).clone(),
        defaults: Settings::default(),
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
    page: Option<u64>,
    page_size: Option<u64>,
    cache: Option<bool>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema)]
pub struct Pagination {
    crates: Vec<CrateOverview>,
    page_size: u64,
    page: u64,
}

/// Get paginated list of crates
#[utoipa::path(
    get,
    path = "/crates",
    tag = "ui",
    params(CratesParams),
    responses(
        (status = 200, description = "Paginated crate list", body = Pagination)
    )
)]
pub async fn crates(Query(params): Query<CratesParams>, State(db): DbState) -> Json<Pagination> {
    let page_size = params.page_size.unwrap_or(10);
    let page = params.page.unwrap_or(0);
    let cache = params.cache.unwrap_or(false);
    let crates = db
        .get_crate_overview_list(page_size, page_size * page, cache)
        .await
        .unwrap_or_default();

    Json(Pagination {
        crates,
        page_size,
        page,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema, utoipa::IntoParams)]
pub struct SearchParams {
    name: OriginalName,
    cache: Option<bool>,
}

/// Search for crates by name
#[utoipa::path(
    get,
    path = "/search",
    tag = "ui",
    params(SearchParams),
    responses(
        (status = 200, description = "Search results", body = Pagination)
    )
)]
pub async fn search(Query(params): Query<SearchParams>, State(db): DbState) -> Json<Pagination> {
    let crates = db
        .search_in_crate_name(&params.name, params.cache.unwrap_or(false))
        .await
        .unwrap_or_default();
    Json(Pagination {
        page_size: crates.len() as u64,
        page: 0, // Return everything as one page
        crates,
    })
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
        (status = 404, description = "Crate not found")
    )
)]
pub async fn crate_data(
    Query(params): Query<CrateDataParams>,
    State(db): DbState,
) -> Result<Json<CrateData>, StatusCode> {
    let index_name = NormalizedName::from(params.name);
    match db.get_crate_data(&index_name).await {
        Ok(cd) => Ok(Json(cd)),
        Err(e) => match e {
            DbError::CrateNotFound(_) => Err(StatusCode::NOT_FOUND),
            _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
        },
    }
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
        (status = 200, description = "Crates.io crate data", body = String),
        (status = 404, description = "Crate not found")
    )
)]
pub async fn cratesio_data(
    State(settings): SettingsState,
    Query(params): Query<CratesIoDataParams>,
) -> Result<String, StatusCode> {
    let url = settings
        .proxy
        .api
        .join(&params.name)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let client = kellnr_common::cratesio_downloader::build_client(
        &settings.proxy.user_agent,
        Duration::from_secs(settings.proxy.connect_timeout_seconds),
        Duration::from_secs(settings.proxy.request_timeout_seconds),
    );
    let req = client.get(url).header("Accept", "application/json");
    let resp = req.send().await;

    match resp {
        Ok(resp) => match resp.status() {
            StatusCode::OK => {
                let data = resp.text().await;
                match data {
                    Ok(data) => Ok(data),
                    Err(e) => {
                        error!("Failed to parse crates.io data: {e}");
                        Err(StatusCode::INTERNAL_SERVER_ERROR)
                    }
                }
            }
            StatusCode::NOT_FOUND => Err(StatusCode::NOT_FOUND),
            _ => {
                error!("Failed to get crates.io data: {}", resp.status());
                Err(StatusCode::NOT_FOUND)
            }
        },
        Err(e) => {
            error!("Failed to get crates.io data: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema)]
pub struct DeleteCrateVersionParams {
    name: OriginalName,
    version: Version,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, ToSchema)]
pub struct DeleteCrateParams {
    name: OriginalName,
}

/// Helper function to delete crate versions from db, storage, and docs.
/// If `versions` is `None`, all versions of the crate are deleted.
async fn delete_crate_versions_impl(
    state: &kellnr_appstate::AppStateData,
    name: &OriginalName,
    versions: Option<Vec<Version>>,
) -> Result<(), RouteError> {
    let versions_to_delete = if let Some(v) = versions {
        v
    } else {
        let crate_meta = state.db.get_crate_meta_list(&name.to_normalized()).await?;
        crate_meta
            .iter()
            .map(|cm| Version::from_unchecked_str(&cm.version))
            .collect()
    };

    for version in &versions_to_delete {
        if let Err(e) = state.db.delete_crate(&name.to_normalized(), version).await {
            error!("Failed to delete crate from database: {e:?}");
            return Err(RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR));
        }

        if let Err(e) = state.crate_storage.delete(name, version).await {
            error!("Failed to delete crate from storage: {e}");
            return Err(RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR));
        }

        if let Err(e) = kellnr_docs::delete(name, version, &state.settings).await {
            error!("Failed to delete crate from docs: {e}");
            return Err(RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR));
        }
    }

    Ok(())
}

pub async fn delete_version(
    Query(params): Query<DeleteCrateVersionParams>,
    _user: AdminUser,
    State(state): AppState,
) -> Result<(), RouteError> {
    delete_crate_versions_impl(&state, &params.name, Some(vec![params.version])).await
}

pub async fn delete_crate(
    Query(params): Query<DeleteCrateParams>,
    _user: AdminUser,
    State(state): AppState,
) -> Result<(), RouteError> {
    delete_crate_versions_impl(&state, &params.name, None).await
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
) -> Result<(), RouteError> {
    let name = OriginalName::try_from(name.as_str())
        .map_err(|_| RouteError::Status(StatusCode::BAD_REQUEST))?;
    let version = Version::try_from(version.as_str())
        .map_err(|_| RouteError::Status(StatusCode::BAD_REQUEST))?;

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
) -> Result<(), RouteError> {
    let name = OriginalName::try_from(name.as_str())
        .map_err(|_| RouteError::Status(StatusCode::BAD_REQUEST))?;

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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct TopCrates {
    pub first: (String, u64),
    pub second: (String, u64),
    pub third: (String, u64),
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
pub async fn statistic(State(db): DbState, State(settings): SettingsState) -> Json<Statistic> {
    fn extract(tops: &[(String, u64)], i: usize) -> (String, u64) {
        if tops.len() > i {
            tops[i].clone()
        } else {
            (String::new(), 0)
        }
    }

    let num_crates = db.get_total_unique_crates().await.unwrap_or_default();
    let num_crate_versions = db.get_total_crate_versions().await.unwrap_or_default();
    let num_crate_downloads = db.get_total_downloads().await.unwrap_or_default();
    let tops = db.get_top_crates_downloads(3).await.unwrap_or_default();
    let num_proxy_crates = db
        .get_total_unique_cached_crates()
        .await
        .unwrap_or_default();
    let num_proxy_crate_versions = db
        .get_total_cached_crate_versions()
        .await
        .unwrap_or_default();
    let num_proxy_crate_downloads = db.get_total_cached_downloads().await.unwrap_or_default();
    let last_updated_crate = db.get_last_updated_crate().await.unwrap_or_default();

    Json(Statistic {
        num_crates,
        num_crate_versions,
        num_crate_downloads,
        num_proxy_crates,
        num_proxy_crate_versions,
        num_proxy_crate_downloads,
        top_crates: TopCrates {
            first: extract(&tops, 0),
            second: extract(&tops, 1),
            third: extract(&tops, 2),
        },
        last_updated_crate,
        proxy_enabled: settings.proxy.enabled,
    })
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
/// Requires ownership of the crate or admin access.
#[utoipa::path(
    post,
    path = "/builds",
    tag = "docs",
    params(BuildParams),
    responses(
        (status = 200, description = "Build queued successfully"),
        (status = 400, description = "Crate or version does not exist"),
        (status = 401, description = "Not authorized or not an owner")
    ),
    security(("session_cookie" = []))
)]
pub async fn build_rustdoc(
    Query(params): Query<BuildParams>,
    State(state): AppState,
    user: MaybeUser,
) -> Result<(), StatusCode> {
    if !state.settings.docs.enabled {
        return Err(StatusCode::BAD_REQUEST);
    }

    let normalized_name = NormalizedName::from(params.package);
    let db = state.db;
    let version = params.version;

    // Check if crate with the version exists.
    if let Some(id) = db
        .get_crate_id(&normalized_name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        if !db
            .crate_version_exists(id, &version)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            return Err(StatusCode::BAD_REQUEST);
        }
    } else {
        return Err(StatusCode::BAD_REQUEST);
    }

    // If the user is the owner of the crate or any admin user,
    // the build operation is allowed.
    let is_allowed = match user {
        MaybeUser::Normal(user) => db
            .is_owner(&normalized_name, &user)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        MaybeUser::Admin(_) => true,
    };

    if !is_allowed {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Add to build queue
    db.add_doc_queue(
        &normalized_name,
        &version,
        &state
            .crate_storage
            .create_rand_doc_queue_path()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::routing::{get, post};
    use axum_extra::extract::cookie::Key;
    use http_body_util::BodyExt;
    use hyper::{Request, header};
    use kellnr_appstate::AppStateData;
    use kellnr_common::crate_data::{CrateRegistryDep, CrateVersionData};
    use kellnr_db::User;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_settings::{Postgresql, Settings, constants};
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
    use mockall::predicate::*;
    use tower::ServiceExt;

    use super::*;
    use crate::test_helper::encode_cookies;

    #[tokio::test]
    async fn settings_no_admin_returns_unauthorized() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .returning(|_| Ok(("admin".to_string(), true)));

        let (settings, storage) = test_deps();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/settings").body(Body::empty()).unwrap())
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn settings_returns_from_settings() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .returning(|_| Ok(("admin".to_string(), true)));

        let (settings, storage) = test_deps();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::get("/settings")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_response = serde_json::from_slice::<SettingsResponse>(&result_msg).unwrap();

        // Set the password to empty string because it is not serialized
        let tmp = kellnr_settings::test_settings();
        let psq = Postgresql {
            pwd: String::default(),
            ..tmp.postgresql
        };
        let expected_state = Settings {
            postgresql: psq,
            ..tmp
        };

        assert_eq!(result_status, StatusCode::OK);
        assert_eq!(result_response.settings, expected_state);
        // `sources` is structurally part of `SettingsResponse` — successful
        // deserialization on line above is sufficient proof it round-trips.
        // The companion test below asserts that an override actually flows
        // into the `sources` map.
    }

    #[tokio::test]
    async fn settings_response_sources_reflect_prov_overrides() {
        use kellnr_settings::{Config, ConfigSource, SettingsProv};

        let mut mock_db = MockDb::new();
        mock_db
            .expect_validate_session()
            .returning(|_| Ok(("admin".to_string(), true)));

        let (settings, storage) = test_deps();

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

        let app: Router = Router::new()
            .route("/settings", get(crate::ui::settings))
            .with_state(AppStateData {
                db: Arc::new(mock_db),
                signing_key: Key::from(TEST_KEY),
                settings: Arc::new(settings.clone()),
                settings_prov: Arc::new(prov),
                crate_storage: Arc::new(KellnrCrateStorage::new(&settings, storage)),
                ..kellnr_appstate::test_state()
            });

        let r = app
            .oneshot(
                Request::get("/settings")
                    .header(
                        header::COOKIE,
                        encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, "token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
        let body = r.into_body().collect().await.unwrap().to_bytes();
        let response: SettingsResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            response.sources.get("registry.data_dir"),
            Some(&ConfigSource::Toml),
            "TOML-set leaf must report ConfigSource::Toml"
        );
        // An untouched leaf must surface as Default — confirms the map covers
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
            "cli_flag comes from clap reflection — long + short"
        );

        let docs_enabled = response
            .leaves
            .iter()
            .find(|l| l.key == "docs.enabled")
            .expect("docs.enabled leaf must be present");
        assert_eq!(docs_enabled.kind, LeafKind::Boolean);
        assert_eq!(docs_enabled.source, ConfigSource::Default);
        assert_eq!(docs_enabled.default, serde_json::json!(false));

        // Secret leaves carry the secret flag — the UI uses it to mask the
        // value before rendering.
        let secret_leaf = response
            .leaves
            .iter()
            .find(|l| l.key == "s3.secret_key")
            .expect("s3.secret_key leaf must be present");
        assert!(secret_leaf.secret, "s3.secret_key must be flagged secret");
    }

    #[tokio::test]
    async fn docs_enabled_no_auth_returns_ok() {
        let mock_db = MockDb::new();
        let (settings, storage) = test_deps();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/docs_enabled").body(Body::empty()).unwrap())
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn docs_enabled_returns_false_by_default() {
        let mock_db = MockDb::new();
        let (settings, storage) = test_deps();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/docs_enabled").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result = serde_json::from_slice::<DocsEnabledResponse>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert!(!result.enabled);
    }

    #[tokio::test]
    async fn docs_enabled_returns_true_when_enabled() {
        let mock_db = MockDb::new();
        let (mut settings, storage) = test_deps();
        settings.docs.enabled = true;
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/docs_enabled").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result = serde_json::from_slice::<DocsEnabledResponse>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert!(result.enabled);
    }

    #[tokio::test]
    async fn build_rust_doc_crate_not_found() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(None));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), false)));
        let (mut settings, storage) = test_deps();
        settings.docs.enabled = true;
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn build_rust_doc_version_not_found() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(Some(1)));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), false)));
        mock_db
            .expect_crate_version_exists()
            .with(eq(1), eq("1.0.0"))
            .returning(move |_, _| Ok(false));
        let (mut settings, storage) = test_deps();
        settings.docs.enabled = true;
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn build_rust_doc_not_owner() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(Some(1)));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), false)));
        mock_db
            .expect_crate_version_exists()
            .with(eq(1), eq("1.0.0"))
            .returning(move |_, _| Ok(true));
        mock_db
            .expect_is_owner()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq("user"),
            )
            .returning(move |_, _| Ok(false));
        mock_db
            .expect_get_user()
            .with(eq("user"))
            .returning(move |_| {
                Ok(User {
                    id: 0,
                    name: "user".to_string(),
                    pwd: String::new(),
                    salt: String::new(),
                    is_admin: false,
                    is_read_only: false,
                    created: String::new(),
                })
            });
        let (mut settings, storage) = test_deps();
        settings.docs.enabled = true;
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn build_rust_doc_is_owner() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(Some(1)));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), false)));
        mock_db
            .expect_crate_version_exists()
            .with(eq(1), eq("1.0.0"))
            .returning(move |_, _| Ok(true));
        mock_db
            .expect_is_owner()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq("user"),
            )
            .returning(move |_, _| Ok(true));
        mock_db
            .expect_get_user()
            .with(eq("user"))
            .returning(move |_| {
                Ok(User {
                    id: 0,
                    name: "user".to_string(),
                    pwd: String::new(),
                    salt: String::new(),
                    is_admin: false,
                    is_read_only: false,
                    created: String::new(),
                })
            });
        mock_db
            .expect_add_doc_queue()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq(Version::try_from("1.0.0").unwrap()),
                always(),
            )
            .times(1)
            .returning(move |_, _, _| Ok(()));

        let (mut settings, storage) = test_deps();
        settings.docs.enabled = true;
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn build_rust_doc_not_owner_but_admin() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .with(eq(NormalizedName::from_unchecked("foobar".to_string())))
            .returning(move |_| Ok(Some(1)));
        mock_db
            .expect_validate_session()
            .with(eq("cookie"))
            .returning(move |_| Ok(("user".to_string(), true)));
        mock_db
            .expect_crate_version_exists()
            .with(eq(1), eq("1.0.0"))
            .returning(move |_, _| Ok(true));
        mock_db
            .expect_is_owner()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq("user"),
            )
            .returning(move |_, _| Ok(false));
        mock_db
            .expect_get_user()
            .with(eq("user"))
            .returning(move |_| {
                Ok(User {
                    id: 0,
                    name: "user".to_string(),
                    pwd: String::new(),
                    salt: String::new(),
                    is_admin: true,
                    is_read_only: false,
                    created: String::new(),
                })
            });
        mock_db
            .expect_add_doc_queue()
            .with(
                eq(NormalizedName::from_unchecked("foobar".to_string())),
                eq(Version::try_from("1.0.0").unwrap()),
                always(),
            )
            .times(1)
            .returning(move |_, _, _| Ok(()));

        let (mut settings, storage) = test_deps();
        settings.docs.enabled = true;
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::post("/build?package=foobar&version=1.0.0")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "token")
                .header(
                    header::COOKIE,
                    encode_cookies([(constants::COOKIE_SESSION_ID, "cookie")]),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn statistic_returns_sparse_statistics() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_total_unique_crates()
            .returning(move || Err(DbError::FailedToCountCrates));
        mock_db
            .expect_get_total_crate_versions()
            .returning(move || Err(DbError::FailedToCountCrateVersions));
        mock_db
            .expect_get_total_downloads()
            .returning(move || Err(DbError::FailedToCountTotalDownloads));
        mock_db
            .expect_get_top_crates_downloads()
            .with(eq(3))
            .returning(move |_| Ok(vec![("top1".to_string(), 1000)]));
        mock_db
            .expect_get_last_updated_crate()
            .returning(move || Ok(None));
        mock_db
            .expect_get_total_unique_cached_crates()
            .returning(move || Err(DbError::FailedToCountCrates));
        mock_db
            .expect_get_total_cached_crate_versions()
            .returning(move || Err(DbError::FailedToCountCrateVersions));
        mock_db
            .expect_get_total_cached_downloads()
            .returning(move || Err(DbError::FailedToCountTotalDownloads));

        let (settings, storage) = test_deps();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/statistic").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_stat = serde_json::from_slice::<Statistic>(&result_msg).unwrap();

        let expect = Statistic {
            num_crates: 0,
            num_crate_versions: 0,
            num_crate_downloads: 0,
            num_proxy_crates: 0,
            num_proxy_crate_versions: 0,
            num_proxy_crate_downloads: 0,
            top_crates: TopCrates {
                first: ("top1".to_string(), 1000),
                second: (String::new(), 0),
                third: (String::new(), 0),
            },
            last_updated_crate: None,
            proxy_enabled: false,
        };

        assert_eq!(expect, result_stat);
    }

    #[tokio::test]
    async fn statistic_returns_empty_statistics() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_total_unique_crates()
            .returning(move || Err(DbError::FailedToCountCrates));
        mock_db
            .expect_get_total_crate_versions()
            .returning(move || Err(DbError::FailedToCountCrateVersions));
        mock_db
            .expect_get_total_downloads()
            .returning(move || Err(DbError::FailedToCountTotalDownloads));
        mock_db
            .expect_get_top_crates_downloads()
            .with(eq(3))
            .returning(move |_| Err(DbError::FailedToCountTotalDownloads));
        mock_db
            .expect_get_last_updated_crate()
            .returning(move || Ok(None));
        mock_db
            .expect_get_total_unique_cached_crates()
            .returning(move || Err(DbError::FailedToCountCrates));
        mock_db
            .expect_get_total_cached_crate_versions()
            .returning(move || Err(DbError::FailedToCountCrateVersions));
        mock_db
            .expect_get_total_cached_downloads()
            .returning(move || Err(DbError::FailedToCountTotalDownloads));

        let (settings, storage) = test_deps();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/statistic").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_stat = serde_json::from_slice::<Statistic>(&result_msg).unwrap();

        let expect = Statistic {
            num_crates: 0,
            num_crate_versions: 0,
            num_crate_downloads: 0,
            num_proxy_crates: 0,
            num_proxy_crate_versions: 0,
            num_proxy_crate_downloads: 0,
            top_crates: TopCrates {
                first: (String::new(), 0),
                second: (String::new(), 0),
                third: (String::new(), 0),
            },
            last_updated_crate: None,
            proxy_enabled: false,
        };

        assert_eq!(expect, result_stat);
    }

    #[tokio::test]
    async fn statistic_returns_crate_statistics() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_total_unique_crates()
            .returning(move || Ok(1000));
        mock_db
            .expect_get_total_crate_versions()
            .returning(move || Ok(10000));
        mock_db
            .expect_get_total_downloads()
            .returning(move || Ok(100_000));
        mock_db
            .expect_get_top_crates_downloads()
            .with(eq(3))
            .returning(move |_| {
                Ok(vec![
                    ("top1".to_string(), 1000),
                    ("top2".to_string(), 500),
                    ("top3".to_string(), 100),
                ])
            });
        mock_db
            .expect_get_total_unique_cached_crates()
            .returning(move || Ok(9999));
        mock_db
            .expect_get_total_cached_crate_versions()
            .returning(move || Ok(99999));
        mock_db
            .expect_get_total_cached_downloads()
            .returning(move || Ok(999_999));
        mock_db.expect_get_last_updated_crate().returning(move || {
            Ok(Some((
                OriginalName::from_unchecked("foobar".to_string()),
                Version::try_from("1.0.0").unwrap(),
            )))
        });

        let (settings, storage) = test_deps();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/statistic").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_stat = serde_json::from_slice::<Statistic>(&result_msg).unwrap();

        let expect = Statistic {
            num_crates: 1000,
            num_crate_versions: 10000,
            num_crate_downloads: 100_000,
            num_proxy_crates: 9999,
            num_proxy_crate_versions: 99999,
            num_proxy_crate_downloads: 999_999,
            top_crates: TopCrates {
                first: ("top1".to_string(), 1000),
                second: ("top2".to_string(), 500),
                third: ("top3".to_string(), 100),
            },
            last_updated_crate: Some((
                OriginalName::from_unchecked("foobar".to_string()),
                Version::try_from("1.0.0").unwrap(),
            )),
            proxy_enabled: false,
        };
        assert_eq!(expect, result_stat);
    }

    #[tokio::test]
    async fn kellnr_version_returns_version() {
        let (settings, storage) = test_deps();
        let mock_db = MockDb::new();

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/version").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_version = serde_json::from_slice::<KellnrVersion>(&result_msg).unwrap();

        assert_eq!("0.0.0-unknown", result_version.version);
    }

    #[tokio::test]
    async fn search_not_hits_returns_nothing() {
        let mut mock_db = MockDb::new();
        let (settings, storage) = test_deps();

        mock_db
            .expect_search_in_crate_name()
            .with(eq("doesnotexist"), eq(false))
            .returning(move |_name, _| Ok(vec![]));

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::get("/search?name=doesnotexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_crates = serde_json::from_slice::<Pagination>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(0, result_crates.crates.len());
        assert_eq!(0, result_crates.page);
        assert_eq!(0, result_crates.page_size);
    }

    #[tokio::test]
    async fn search_returns_only_searched_results() {
        let mut mock_db = MockDb::new();
        let (settings, storage) = test_deps();

        let test_crate_summary = CrateOverview {
            name: "hello".to_string(),
            version: "1.0.0".to_string(),
            date: "12-10-2021 05:41:00".to_string(),
            total_downloads: 2,
            ..Default::default()
        };

        let tc = test_crate_summary.clone();
        mock_db
            .expect_search_in_crate_name()
            .with(eq("hello"), eq(false))
            .returning(move |_, _| Ok(vec![tc.clone()]));

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::get("/search?name=hello")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_crates = serde_json::from_slice::<Pagination>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(1, result_crates.crates.len());
        assert_eq!(0, result_crates.page);
        assert_eq!(1, result_crates.page_size);
        assert_eq!(test_crate_summary, result_crates.crates[0]);
    }

    #[tokio::test]
    async fn crate_get_crate_information() {
        let mut mock_db = MockDb::new();
        let (settings, storage) = test_deps();

        let expected_crate_data = CrateData {
            name: "crate1".to_string(),
            owners: vec!["owner1".to_string(), "owner2".to_string()],
            max_version: "1.0.0".to_string(),
            total_downloads: 5,
            last_updated: "12-10-2021 05:41:00".to_string(),
            homepage: Some("homepage".to_string()),
            description: Some("description".to_string()),
            categories: vec!["cat1".to_string(), "cat2".to_string()],
            keywords: vec!["key1".to_string(), "key2".to_string()],
            authors: vec!["author1".to_string(), "author2".to_string()],
            repository: Some("repository".to_string()),
            versions: vec![CrateVersionData {
                version: "1.0.0".to_string(),
                created: "12-10-2021 05:41:00".to_string(),
                downloads: 5,
                readme: Some("readme".to_string()),
                license: Some("MIT".to_string()),
                license_file: Some("license".to_string()),
                documentation: Some("documentation".to_string()),
                dependencies: vec![CrateRegistryDep {
                    name: "dep1".to_string(),
                    description: Some("description".to_string()),
                    version_req: "1.0.0".to_string(),
                    target: Some("target".to_string()),
                    kind: Some("dev".to_string()),
                    registry: Some("registry".to_string()),
                    ..Default::default()
                }],
                checksum: "checksum".to_string(),
                features: BTreeMap::default(),
                yanked: false,
                links: Some("links".to_string()),
                v: 1,
            }],
        };

        let ecd = expected_crate_data.clone();
        mock_db
            .expect_get_crate_data()
            .returning(move |_| Ok(ecd.clone()));

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::get("/crate_data?name=crate1&version=1.0.0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_crate_data = serde_json::from_slice::<CrateData>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(expected_crate_data, result_crate_data);
    }

    #[tokio::test]
    async fn crates_get_page() {
        let mut mock_db = MockDb::new();
        let (settings, storage) = test_deps();

        let test_crate_overview = CrateOverview {
            name: "c1".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            total_downloads: 2,
            date: "12-10-2021 05:41:00".to_string(),
            documentation: None,
            is_cache: false,
        };

        let test_crates = std::iter::repeat_with(|| test_crate_overview.clone())
            .take(10)
            .collect::<Vec<_>>();

        let tc = test_crates.clone();

        mock_db
            .expect_get_crate_overview_list()
            .with(eq(10), eq(0), eq(false))
            .returning(move |_, _, _| Ok(tc.clone()));

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/crates?page=0").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_pagination = serde_json::from_slice::<Pagination>(&result_msg).unwrap();

        let expected = test_crates[0..10].to_vec();
        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(0, result_pagination.page);
        assert_eq!(10, result_pagination.page_size);
        assert_eq!(10, result_pagination.crates.len());
        assert_eq!(expected, result_pagination.crates);
    }

    #[tokio::test]
    async fn crates_get_all_crates() {
        let mut mock_db = MockDb::new();
        let (settings, storage) = test_deps();

        let expected_crate_overview = vec![
            CrateOverview {
                name: "c1".to_string(),
                version: "1.0.0".to_string(),
                date: "12-11-2021 05:41:00".to_string(),
                total_downloads: 1,
                description: Some("Desc".to_string()),
                documentation: Some("Docs".to_string()),
                is_cache: true,
            },
            CrateOverview {
                name: "c2".to_string(),
                version: "2.0.0".to_string(),
                date: "12-12-2021 05:41:00".to_string(),
                total_downloads: 2,
                description: Some("Desc".to_string()),
                documentation: Some("Docs".to_string()),
                is_cache: true,
            },
            CrateOverview {
                name: "c3".to_string(),
                version: "3.0.0".to_string(),
                date: "12-09-2021 05:41:00".to_string(),
                total_downloads: 3,
                description: None,
                documentation: None,
                is_cache: true,
            },
        ];

        let crate_overview = expected_crate_overview.clone();
        mock_db
            .expect_get_crate_overview_list()
            .with(eq(10), eq(0), eq(false))
            .returning(move |_, _, _| Ok(crate_overview.clone()));

        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(Request::get("/crates").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let result_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let result_pagination = serde_json::from_slice::<Pagination>(&result_msg).unwrap();

        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(3, result_pagination.crates.len());
        assert_eq!(0, result_pagination.page);
        assert_eq!(10, result_pagination.page_size);
        assert_eq!(expected_crate_overview, result_pagination.crates);
    }

    #[tokio::test]
    async fn cratesio_data_returns_data() {
        let mock_db = MockDb::new();
        let (settings, storage) = test_deps();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::get("/cratesio_data?name=quote")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        let result_status = r.status();
        let body =
            String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
        assert!(body.contains("quote"));
        assert_eq!(StatusCode::OK, result_status);
    }

    #[tokio::test]
    async fn cratesio_data_not_found() {
        let mock_db = MockDb::new();
        let (settings, storage) = test_deps();
        let r = app(
            mock_db,
            KellnrCrateStorage::new(&settings, storage),
            settings,
        )
        .oneshot(
            Request::get("/cratesio_data?name=thisdoesnotevenexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    fn test_deps() -> (Settings, DynStorage) {
        let settings = kellnr_settings::test_settings();
        let storage = FSStorage::new(&settings.crates_path()).unwrap();
        let storage = Box::new(storage) as DynStorage;
        (settings, storage)
    }

    const TEST_KEY: &[u8] = &[1; 64];
    fn app(mock_db: MockDb, crate_storage: KellnrCrateStorage, settings: Settings) -> Router {
        Router::new()
            .route("/search", get(search))
            .route("/crates", get(crates))
            .route("/crate_data", get(crate_data))
            .route("/version", get(kellnr_version))
            .route("/statistic", get(statistic))
            .route("/build", post(build_rustdoc))
            .route("/cratesio_data", get(cratesio_data))
            .route("/settings", get(crate::ui::settings))
            .route("/docs_enabled", get(docs_enabled))
            .with_state(AppStateData {
                db: Arc::new(mock_db),
                signing_key: Key::from(TEST_KEY),
                settings: Arc::new(settings),
                crate_storage: Arc::new(crate_storage),
                ..kellnr_appstate::test_state()
            })
    }
}
