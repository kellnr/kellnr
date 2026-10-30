use std::time::Duration;

use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

use crate::compile_time_config;
use crate::constants::MAX_SESSION_AGE;

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

    /// Session lifetime in seconds, counted from the login (0 = sessions
    /// never expire)
    #[arg(long = "registry-session-age")]
    pub session_age_seconds: u64,

    /// Max number of crates in the in-memory crate cache (0 = disabled)
    pub cache_size: u64,

    /// Max total size of the in-memory crate cache in MB (0 = no size limit,
    /// only `cache_size` applies)
    pub cache_max_size: u64,

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

    /// Cookie signing key (for multi-instance setups). At least 64 bytes of
    /// random data; generate it with e.g. `openssl rand -hex 64`. Unset, a
    /// random key is generated on every start
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

    /// Password attempts allowed per username within the window before
    /// further attempts are rejected with HTTP 429 (0 = disabled). Applies to
    /// web UI login and cargo basic auth; a successful login resets the count
    pub auth_max_failed_attempts: u32,

    /// Window in seconds for counting password attempts per username
    #[arg(long = "registry-auth-failed-attempts-window")]
    pub auth_failed_attempts_window_seconds: u64,

    /// Show links to sponsor Kellnr in the web UI and the startup log
    pub sponsor_links_enabled: bool,
}

impl Registry {
    /// The lifetime of a session. Zero (sessions never expire) and values
    /// above [`MAX_SESSION_AGE`] resolve to [`MAX_SESSION_AGE`].
    pub fn session_max_age(&self) -> Duration {
        if self.session_age_seconds == 0 {
            MAX_SESSION_AGE
        } else {
            Duration::from_secs(self.session_age_seconds).min(MAX_SESSION_AGE)
        }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            session_age_seconds: 60 * 60 * 8,
            cache_size: 1000,
            cache_max_size: 0,
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
            auth_max_failed_attempts: 10,
            auth_failed_attempts_window_seconds: 300,
            sponsor_links_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(session_age_seconds: u64) -> Registry {
        Registry {
            session_age_seconds,
            ..Registry::default()
        }
    }

    #[test]
    fn session_max_age_is_the_configured_age() {
        assert_eq!(registry(60).session_max_age(), Duration::from_mins(1));
    }

    #[test]
    fn session_max_age_is_bounded() {
        assert_eq!(registry(0).session_max_age(), MAX_SESSION_AGE);
        assert_eq!(registry(u64::MAX).session_max_age(), MAX_SESSION_AGE);
    }
}
