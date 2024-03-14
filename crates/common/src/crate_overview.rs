use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromQueryResult)]
pub struct CrateOverview {
    pub name: String,
    pub version: String,
    pub date: String,
    pub total_downloads: i64,
    pub description: Option<String>,
    pub documentation: Option<String>,
    pub is_cache: bool,
}
