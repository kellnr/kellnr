use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PubDataSuccess {
    pub warnings: Option<Warnings>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
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
