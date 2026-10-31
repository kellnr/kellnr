use kellnr_entity::group;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::GroupId;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Group {
    #[serde(skip_serializing)]
    pub id: GroupId,
    pub name: String,
}

impl From<group::Model> for Group {
    fn from(g: group::Model) -> Self {
        Self {
            id: GroupId::new(g.id),
            name: g.name,
        }
    }
}
