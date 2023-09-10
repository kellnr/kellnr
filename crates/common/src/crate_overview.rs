use rocket::serde::{Deserialize, Serialize};
use sea_orm::FromQueryResult;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromQueryResult)]
pub struct CrateOverview {
    pub original_name: String,
    pub max_version: String,
    pub last_updated: String,
    pub total_downloads: i64,
    pub description: Option<String>,
    pub documentation: Option<String>,
}
