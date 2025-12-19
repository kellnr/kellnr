use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
#[serde(default)]
pub struct Docs {
    pub enabled: bool,
    pub max_size: usize,
}

impl Default for Docs {
    fn default() -> Self {
        Self {
            enabled: false,
            max_size: 100,
        }
    }
}
