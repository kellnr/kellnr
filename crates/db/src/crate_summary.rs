use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateSummary {
    pub name: String,
    pub max_version: String,
    pub last_updated: String,
    pub total_downloads: i64,
}
