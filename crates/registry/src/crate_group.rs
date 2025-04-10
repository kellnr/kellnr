use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateGroup {
    pub id: i32,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateGroupList {
    pub groups: Vec<CrateGroup>,
}

impl From<Vec<CrateGroup>> for CrateGroupList {
    fn from(groups: Vec<CrateGroup>) -> Self {
        Self { groups }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateGroupRequest {
    pub groups: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
