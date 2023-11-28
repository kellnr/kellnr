use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Proxy {
    pub enabled: bool,
    pub num_threads: usize,
}

impl Default for Proxy {
    fn default() -> Self {
        Self { enabled: false, num_threads: 10 }
    }
}
