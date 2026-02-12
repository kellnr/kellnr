use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateVersion {
    pub version: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateVersionList {
    pub versions: Vec<CrateVersion>,
}

impl From<Vec<CrateVersion>> for CrateVersionList {
    fn from(versions: Vec<CrateVersion>) -> Self {
        Self { versions }
    }
}
