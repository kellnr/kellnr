use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Registry {
    pub data_dir: String,
    pub session_age_seconds: u64,
    pub cache_size: u64,
    pub max_crate_size: u64,
    pub max_db_connections: u32,
    pub auth_required: bool,
    pub required_crate_fields: Vec<String>,
    pub new_crates_restricted: bool,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            data_dir: String::from("/tmp/kellnr"),
            session_age_seconds: 60 * 60 * 8,
            cache_size: 1000,
            max_crate_size: 10 * 1000,
            max_db_connections: 0,
            auth_required: false,
            required_crate_fields: Vec::new(),
            new_crates_restricted: false,
        }
    }
}
