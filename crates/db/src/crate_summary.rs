use kellnr_entity::krate;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateSummary {
    pub name: String,
    pub max_version: String,
    pub last_updated: String,
    pub total_downloads: i64,
}

impl From<krate::Model> for CrateSummary {
    fn from(c: krate::Model) -> Self {
        Self {
            name: c.name,
            max_version: c.max_version,
            last_updated: c.last_updated,
            total_downloads: c.total_downloads,
        }
    }
}
