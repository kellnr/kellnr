use serde::{Deserialize, Serialize};

use crate::CrateId;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crate {
    #[serde(skip_serializing)]
    pub id: CrateId,
    pub name: String,
}
