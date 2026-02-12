use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct Toolchain {
    /// Enable toolchain distribution server
    #[default(false)]
    #[arg(id = "toolchain-enabled", long = "toolchain-enabled")]
    pub enabled: bool,

    /// Max toolchain archive size in MB
    #[default(500)]
    #[arg(id = "toolchain-max-size", long = "toolchain-max-size")]
    pub max_size: usize,
}
