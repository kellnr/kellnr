use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crate {
    #[serde(skip_serializing)]
    pub id: i32,
    pub name: String,
}
