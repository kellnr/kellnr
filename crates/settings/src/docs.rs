use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "docs")]
pub struct Docs {
    /// Enable documentation hosting
    pub enabled: bool,

    /// Max docs size in MB
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
