use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct YankSuccess {
    ok: bool,
}

impl YankSuccess {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for YankSuccess {
    fn default() -> Self {
        Self { ok: true }
    }
}
