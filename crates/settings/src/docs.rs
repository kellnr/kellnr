use serde::{Serialize, Deserialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Docs {
    pub enabled: bool,
    pub max_size: usize,
}

impl Default for Docs {
    fn default() -> Self {
        Self { enabled: false, max_size: 100 }
    }
}
