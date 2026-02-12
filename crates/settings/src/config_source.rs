use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the source of a configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSource {
    /// Value comes from compiled-in defaults
    Default,
    /// Value was set in a TOML configuration file
    Toml,
    /// Value was set via environment variable
    Env,
    /// Value was set via command-line argument
    Cli,
}

/// Maps setting keys (e.g., `"registry.data_dir"`) to their configuration source.
pub type SourceMap = HashMap<String, ConfigSource>;

/// Initialize a `SourceMap` with all settings set to Default.
pub fn init_default_sources() -> SourceMap {
    let keys = [
        // Registry settings
        "registry.data_dir",
        "registry.session_age_seconds",
        "registry.cache_size",
        "registry.max_crate_size",
        "registry.max_db_connections",
        "registry.auth_required",
        "registry.required_crate_fields",
        "registry.new_crates_restricted",
        "registry.cookie_signing_key",
        "registry.allow_ownerless_crates",
        "registry.token_cache_enabled",
        "registry.token_cache_ttl_seconds",
        "registry.token_cache_max_capacity",
        "registry.token_db_retry_count",
        "registry.token_db_retry_delay_ms",
        // Local settings
        "local.ip",
        "local.port",
        // Origin settings
        "origin.hostname",
        "origin.port",
        "origin.protocol",
        "origin.path",
        // Log settings
        "log.level",
        "log.format",
        "log.level_web_server",
        // Docs settings
        "docs.enabled",
        "docs.max_size",
        // Proxy settings
        "proxy.enabled",
        "proxy.num_threads",
        "proxy.download_on_update",
        "proxy.url",
        "proxy.index",
        // PostgreSQL settings
        "postgresql.enabled",
        "postgresql.address",
        "postgresql.port",
        "postgresql.db",
        "postgresql.user",
        "postgresql.pwd",
        // S3 settings
        "s3.enabled",
        "s3.access_key",
        "s3.secret_key",
        "s3.region",
        "s3.endpoint",
        "s3.allow_http",
        "s3.crates_bucket",
        "s3.cratesio_bucket",
        "s3.toolchain_bucket",
        // Setup settings
        "setup.admin_pwd",
        "setup.admin_token",
        // OAuth2 settings
        "oauth2.enabled",
        "oauth2.issuer_url",
        "oauth2.client_id",
        "oauth2.client_secret",
        "oauth2.scopes",
        "oauth2.auto_provision_users",
        "oauth2.admin_group_claim",
        "oauth2.admin_group_value",
        "oauth2.read_only_group_claim",
        "oauth2.read_only_group_value",
        "oauth2.button_text",
        // Toolchain settings
        "toolchain.enabled",
        "toolchain.max_size",
    ];

    keys.into_iter()
        .map(|k| (k.to_string(), ConfigSource::Default))
        .collect()
}

/// Environment variable mappings for each setting key.
/// Returns tuples of (`setting_key`, `env_var_name`).
pub fn env_var_mappings() -> &'static [(&'static str, &'static str)] {
    &[
        // Registry settings
        ("registry.data_dir", "KELLNR_REGISTRY__DATA_DIR"),
        (
            "registry.session_age_seconds",
            "KELLNR_REGISTRY__SESSION_AGE_SECONDS",
        ),
        ("registry.cache_size", "KELLNR_REGISTRY__CACHE_SIZE"),
        ("registry.max_crate_size", "KELLNR_REGISTRY__MAX_CRATE_SIZE"),
        (
            "registry.max_db_connections",
            "KELLNR_REGISTRY__MAX_DB_CONNECTIONS",
        ),
        ("registry.auth_required", "KELLNR_REGISTRY__AUTH_REQUIRED"),
        (
            "registry.required_crate_fields",
            "KELLNR_REGISTRY__REQUIRED_CRATE_FIELDS",
        ),
        (
            "registry.new_crates_restricted",
            "KELLNR_REGISTRY__NEW_CRATES_RESTRICTED",
        ),
        (
            "registry.cookie_signing_key",
            "KELLNR_REGISTRY__COOKIE_SIGNING_KEY",
        ),
        (
            "registry.allow_ownerless_crates",
            "KELLNR_REGISTRY__ALLOW_OWNERLESS_CRATES",
        ),
        (
            "registry.token_cache_enabled",
            "KELLNR_REGISTRY__TOKEN_CACHE_ENABLED",
        ),
        (
            "registry.token_cache_ttl_seconds",
            "KELLNR_REGISTRY__TOKEN_CACHE_TTL_SECONDS",
        ),
        (
            "registry.token_cache_max_capacity",
            "KELLNR_REGISTRY__TOKEN_CACHE_MAX_CAPACITY",
        ),
        (
            "registry.token_db_retry_count",
            "KELLNR_REGISTRY__TOKEN_DB_RETRY_COUNT",
        ),
        (
            "registry.token_db_retry_delay_ms",
            "KELLNR_REGISTRY__TOKEN_DB_RETRY_DELAY_MS",
        ),
        // Local settings
        ("local.ip", "KELLNR_LOCAL__IP"),
        ("local.port", "KELLNR_LOCAL__PORT"),
        // Origin settings
        ("origin.hostname", "KELLNR_ORIGIN__HOSTNAME"),
        ("origin.port", "KELLNR_ORIGIN__PORT"),
        ("origin.protocol", "KELLNR_ORIGIN__PROTOCOL"),
        ("origin.path", "KELLNR_ORIGIN__PATH"),
        // Log settings
        ("log.level", "KELLNR_LOG__LEVEL"),
        ("log.format", "KELLNR_LOG__FORMAT"),
        ("log.level_web_server", "KELLNR_LOG__LEVEL_WEB_SERVER"),
        // Docs settings
        ("docs.enabled", "KELLNR_DOCS__ENABLED"),
        ("docs.max_size", "KELLNR_DOCS__MAX_SIZE"),
        // Proxy settings
        ("proxy.enabled", "KELLNR_PROXY__ENABLED"),
        ("proxy.num_threads", "KELLNR_PROXY__NUM_THREADS"),
        (
            "proxy.download_on_update",
            "KELLNR_PROXY__DOWNLOAD_ON_UPDATE",
        ),
        ("proxy.url", "KELLNR_PROXY__URL"),
        ("proxy.index", "KELLNR_PROXY__INDEX"),
        // PostgreSQL settings
        ("postgresql.enabled", "KELLNR_POSTGRESQL__ENABLED"),
        ("postgresql.address", "KELLNR_POSTGRESQL__ADDRESS"),
        ("postgresql.port", "KELLNR_POSTGRESQL__PORT"),
        ("postgresql.db", "KELLNR_POSTGRESQL__DB"),
        ("postgresql.user", "KELLNR_POSTGRESQL__USER"),
        ("postgresql.pwd", "KELLNR_POSTGRESQL__PWD"),
        // S3 settings
        ("s3.enabled", "KELLNR_S3__ENABLED"),
        ("s3.access_key", "KELLNR_S3__ACCESS_KEY"),
        ("s3.secret_key", "KELLNR_S3__SECRET_KEY"),
        ("s3.region", "KELLNR_S3__REGION"),
        ("s3.endpoint", "KELLNR_S3__ENDPOINT"),
        ("s3.allow_http", "KELLNR_S3__ALLOW_HTTP"),
        ("s3.crates_bucket", "KELLNR_S3__CRATES_BUCKET"),
        ("s3.cratesio_bucket", "KELLNR_S3__CRATESIO_BUCKET"),
        ("s3.toolchain_bucket", "KELLNR_S3__TOOLCHAIN_BUCKET"),
        // Setup settings
        ("setup.admin_pwd", "KELLNR_SETUP__ADMIN_PWD"),
        ("setup.admin_token", "KELLNR_SETUP__ADMIN_TOKEN"),
        // OAuth2 settings
        ("oauth2.enabled", "KELLNR_OAUTH2__ENABLED"),
        ("oauth2.issuer_url", "KELLNR_OAUTH2__ISSUER_URL"),
        ("oauth2.client_id", "KELLNR_OAUTH2__CLIENT_ID"),
        ("oauth2.client_secret", "KELLNR_OAUTH2__CLIENT_SECRET"),
        ("oauth2.scopes", "KELLNR_OAUTH2__SCOPES"),
        (
            "oauth2.auto_provision_users",
            "KELLNR_OAUTH2__AUTO_PROVISION_USERS",
        ),
        (
            "oauth2.admin_group_claim",
            "KELLNR_OAUTH2__ADMIN_GROUP_CLAIM",
        ),
        (
            "oauth2.admin_group_value",
            "KELLNR_OAUTH2__ADMIN_GROUP_VALUE",
        ),
        (
            "oauth2.read_only_group_claim",
            "KELLNR_OAUTH2__READ_ONLY_GROUP_CLAIM",
        ),
        (
            "oauth2.read_only_group_value",
            "KELLNR_OAUTH2__READ_ONLY_GROUP_VALUE",
        ),
        ("oauth2.button_text", "KELLNR_OAUTH2__BUTTON_TEXT"),
        // Toolchain settings
        ("toolchain.enabled", "KELLNR_TOOLCHAIN__ENABLED"),
        ("toolchain.max_size", "KELLNR_TOOLCHAIN__MAX_SIZE"),
    ]
}

/// Check which environment variables are set and mark them in the source map.
pub fn track_env_sources(sources: &mut SourceMap) {
    for (key, env_var) in env_var_mappings() {
        if std::env::var(env_var).is_ok() {
            sources.insert((*key).to_string(), ConfigSource::Env);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_default_sources() {
        let sources = init_default_sources();
        assert!(sources.len() > 50); // We have many settings
        assert_eq!(
            sources.get("registry.data_dir"),
            Some(&ConfigSource::Default)
        );
        assert_eq!(sources.get("local.port"), Some(&ConfigSource::Default));
    }

    #[test]
    fn test_env_var_mappings_count() {
        let mappings = env_var_mappings();
        assert!(mappings.len() > 50);
    }

    #[test]
    fn test_config_source_serialization() {
        assert_eq!(
            serde_json::to_string(&ConfigSource::Default).unwrap(),
            "\"default\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigSource::Toml).unwrap(),
            "\"toml\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigSource::Env).unwrap(),
            "\"env\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigSource::Cli).unwrap(),
            "\"cli\""
        );
    }
}
