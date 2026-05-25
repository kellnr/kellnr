use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "toolchain")]
pub struct Toolchain {
    /// Enable toolchain distribution server
    pub enabled: bool,

    /// Max toolchain archive size in MB
    pub max_size: usize,
}

impl Default for Toolchain {
    fn default() -> Self {
        Self {
            enabled: false,
            max_size: 500,
        }
    }
}
