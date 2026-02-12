use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

use crate::compile_time_config;

fn default_data_dir() -> String {
    // Priority: runtime env var > compile-time > empty (must be set via CLI or config)
    std::env::var("KELLNR_DATA_DIR")
        .ok()
        .or_else(|| compile_time_config::KELLNR_COMPTIME__DATA_DIR.map(String::from))
        .unwrap_or_default()
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Registry {
    /// Data directory for crates, index, and database
    #[default(default_data_dir())]
    #[arg(id = "registry-data-dir", long = "registry-data-dir", short = 'd')]
    pub data_dir: String,

    /// Session timeout in seconds
    #[default(60 * 60 * 8)]
    #[arg(id = "registry-session-age", long = "registry-session-age")]
    pub session_age_seconds: u64,

    /// Cache size
    #[default(1000)]
    #[arg(id = "registry-cache-size", long = "registry-cache-size")]
    pub cache_size: u64,

    /// Max crate size in KB
    #[default(10 * 1000)]
    #[arg(id = "registry-max-crate-size", long = "registry-max-crate-size")]
    pub max_crate_size: u64,

    /// Max database connections (0 = unlimited)
    #[default(0)]
    #[arg(
        id = "registry-max-db-connections",
        long = "registry-max-db-connections"
    )]
    pub max_db_connections: u32,

    /// Require authentication for all operations
    #[default(false)]
    #[arg(id = "registry-auth-required", long = "registry-auth-required")]
    pub auth_required: bool,

    /// Required crate fields (comma-separated)
    #[default(Vec::new())]
    #[arg(
        id = "registry-required-crate-fields",
        long = "registry-required-crate-fields",
        value_delimiter = ','
    )]
    pub required_crate_fields: Vec<String>,

    /// Restrict new crate uploads to admins
    #[default(false)]
    #[arg(
        id = "registry-new-crates-restricted",
        long = "registry-new-crates-restricted"
    )]
    pub new_crates_restricted: bool,

    /// Cookie signing key (for multi-instance setups)
    #[default(None)]
    #[arg(
        id = "registry-cookie-signing-key",
        long = "registry-cookie-signing-key"
    )]
    pub cookie_signing_key: Option<String>,

    /// Allow crates without owners
    #[default(false)]
    #[arg(
        id = "registry-allow-ownerless-crates",
        long = "registry-allow-ownerless-crates"
    )]
    pub allow_ownerless_crates: bool,

    /// Enable token cache
    #[default(true)]
    #[arg(
        id = "registry-token-cache-enabled",
        long = "registry-token-cache-enabled"
    )]
    pub token_cache_enabled: bool,

    /// Token cache TTL in seconds
    #[default(1800)]
    #[arg(id = "registry-token-cache-ttl", long = "registry-token-cache-ttl")]
    pub token_cache_ttl_seconds: u64,

    /// Token cache max capacity
    #[default(10000)]
    #[arg(
        id = "registry-token-cache-max-capacity",
        long = "registry-token-cache-max-capacity"
    )]
    pub token_cache_max_capacity: u64,

    /// Token DB retry count
    #[default(3)]
    #[arg(
        id = "registry-token-db-retry-count",
        long = "registry-token-db-retry-count"
    )]
    pub token_db_retry_count: u32,

    /// Token DB retry delay in ms
    #[default(100)]
    #[arg(
        id = "registry-token-db-retry-delay",
        long = "registry-token-db-retry-delay"
    )]
    pub token_db_retry_delay_ms: u64,
}
