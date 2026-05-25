use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

use crate::compile_time_config;

fn default_data_dir() -> String {
    // Priority: runtime env var > compile-time > empty (must be set via CLI or config)
    std::env::var("KELLNR_DATA_DIR")
        .ok()
        .or_else(|| compile_time_config::KELLNR_COMPTIME__DATA_DIR.map(String::from))
        .unwrap_or_default()
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "registry")]
#[allow(clippy::struct_excessive_bools)]
pub struct Registry {
    /// Data directory for crates, index, and database
    #[arg(short = 'd')]
    pub data_dir: String,

    /// Session timeout in seconds
    #[arg(long = "registry-session-age")]
    pub session_age_seconds: u64,

    /// Cache size
    pub cache_size: u64,

    /// Max crate size in MB
    pub max_crate_size: u64,

    /// Max database connections (0 = unlimited)
    pub max_db_connections: u32,

    /// Require authentication for all operations
    pub auth_required: bool,

    /// Required crate fields (comma-separated)
    #[configurable(env_list)]
    #[arg(value_delimiter = ',')]
    pub required_crate_fields: Vec<String>,

    /// Restrict new crate uploads to admins
    pub new_crates_restricted: bool,

    /// Cookie signing key (for multi-instance setups)
    pub cookie_signing_key: Option<String>,

    /// Allow crates without owners
    pub allow_ownerless_crates: bool,

    /// Enable token cache
    pub token_cache_enabled: bool,

    /// Token cache TTL in seconds
    #[arg(long = "registry-token-cache-ttl")]
    pub token_cache_ttl_seconds: u64,

    /// Token cache max capacity
    pub token_cache_max_capacity: u64,

    /// Token DB retry count
    pub token_db_retry_count: u32,

    /// Token DB retry delay in ms
    #[arg(long = "registry-token-db-retry-delay")]
    pub token_db_retry_delay_ms: u64,

    /// Download request timeout in seconds (0 = disabled)
    #[arg(long = "registry-download-timeout")]
    pub download_timeout_seconds: u64,

    /// Max concurrent download requests (0 = unlimited)
    pub download_max_concurrent: usize,

    /// Download counter flush interval in seconds (0 = flush every download)
    #[arg(long = "registry-download-counter-flush")]
    pub download_counter_flush_seconds: u64,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            session_age_seconds: 60 * 60 * 8,
            cache_size: 1000,
            max_crate_size: 10,
            max_db_connections: 0,
            auth_required: false,
            required_crate_fields: Vec::new(),
            new_crates_restricted: false,
            cookie_signing_key: None,
            allow_ownerless_crates: false,
            token_cache_enabled: true,
            token_cache_ttl_seconds: 1800,
            token_cache_max_capacity: 10000,
            token_db_retry_count: 3,
            token_db_retry_delay_ms: 100,
            download_timeout_seconds: 60,
            download_max_concurrent: 20,
            download_counter_flush_seconds: 30,
        }
    }
}
