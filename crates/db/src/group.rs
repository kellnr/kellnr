use kellnr_entity::group;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Group {
    #[serde(skip_serializing)]
    pub id: i32,
    pub name: String,
}

impl From<group::Model> for Group {
    fn from(g: group::Model) -> Self {
        Self {
            id: g.id as i32,
            name: g.name,
        }
    }
}
