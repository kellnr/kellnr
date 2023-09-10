use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateMeta {
    pub name: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub id: i64,
    pub version: String,
    pub created: String,
    pub downloads: i64,
    #[serde(skip_serializing, skip_deserializing)]
    pub crate_fk: i64,
}
