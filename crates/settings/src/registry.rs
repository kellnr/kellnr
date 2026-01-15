use serde::{Deserialize, Serialize};

use crate::compile_time_config;

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
#[serde(default)]
pub struct Registry {
    pub data_dir: String,
    pub session_age_seconds: u64,
    pub cache_size: u64,
    pub max_crate_size: u64,
    pub max_db_connections: u32,
    pub auth_required: bool,
    pub required_crate_fields: Vec<String>,
    pub new_crates_restricted: bool,
    pub cookie_signing_key: Option<String>,
    pub allow_ownerless_crates: bool,
    pub token_cache_enabled: bool,
    pub token_cache_ttl_seconds: u64,
    pub token_cache_max_capacity: u64,
    pub token_db_retry_count: u32,
    pub token_db_retry_delay_ms: u64,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            data_dir: compile_time_config::KELLNR_DATA_DIR.to_string(),
            session_age_seconds: 60 * 60 * 8,
            cache_size: 1000,
            max_crate_size: 10 * 1000,
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
        }
    }
}
