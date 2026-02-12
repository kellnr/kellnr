use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct EmptyCrateSuccess {
    ok: bool,
}
impl EmptyCrateSuccess {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for EmptyCrateSuccess {
    fn default() -> Self {
        Self { ok: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, ToSchema)]
pub struct PubDataSuccess {
    pub warnings: Option<Warnings>,
}

#[derive(Serialize, Deserialize, Debug, Default, ToSchema)]
pub struct Warnings {
    pub invalid_categories: Option<Vec<String>>,
    pub invalid_badges: Option<Vec<String>>,
    pub other: Option<Vec<String>>,
}

impl PubDataSuccess {
    pub fn new() -> Self {
        Self::default()
    }
}
