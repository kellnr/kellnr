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

/// All setting keys used for configuration tracking.
const SETTING_KEYS: &[&str] = &[
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

/// Maps setting keys (e.g., `"registry.data_dir"`) to their configuration source.
pub type SourceMap = HashMap<String, ConfigSource>;

/// Compute the environment variable name for a given setting key.
/// E.g., `"registry.data_dir"` becomes `"KELLNR_REGISTRY__DATA_DIR"`.
fn env_var_name(key: &str) -> String {
    format!("KELLNR_{}", key.to_uppercase().replace('.', "__"))
}

/// Initialize a `SourceMap` with all settings set to Default.
pub fn init_default_sources() -> SourceMap {
    SETTING_KEYS
        .iter()
        .map(|k| (k.to_string(), ConfigSource::Default))
        .collect()
}

/// Check which environment variables are set and mark them in the source map.
pub fn track_env_sources(sources: &mut SourceMap) {
    for key in SETTING_KEYS {
        if std::env::var(env_var_name(key)).is_ok() {
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
    fn test_setting_keys_count() {
        assert!(SETTING_KEYS.len() > 50);
    }

    #[test]
    fn test_env_var_name() {
        assert_eq!(
            env_var_name("registry.data_dir"),
            "KELLNR_REGISTRY__DATA_DIR"
        );
        assert_eq!(env_var_name("local.port"), "KELLNR_LOCAL__PORT");
        assert_eq!(
            env_var_name("oauth2.auto_provision_users"),
            "KELLNR_OAUTH2__AUTO_PROVISION_USERS"
        );
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
