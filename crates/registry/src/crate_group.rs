use kellnr_common::account_name::GroupName;
use kellnr_db::GroupId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateGroup {
    pub id: GroupId,
    pub name: GroupName,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateGroupList {
    pub groups: Vec<CrateGroup>,
}

impl From<Vec<CrateGroup>> for CrateGroupList {
    fn from(groups: Vec<CrateGroup>) -> Self {
        Self { groups }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateGroupRequest {
    pub groups: Vec<GroupName>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateGroupResponse {
    pub ok: bool,
    pub msg: String,
}

impl From<&str> for CrateGroupResponse {
    fn from(msg: &str) -> Self {
        Self {
            ok: true,
            msg: msg.to_string(),
        }
    }
}
