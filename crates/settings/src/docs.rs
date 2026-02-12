use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct Docs {
    /// Enable documentation hosting
    #[default(false)]
    #[arg(id = "docs-enabled", long = "docs-enabled")]
    pub enabled: bool,

    /// Max docs size in MB
    #[default(100)]
    #[arg(id = "docs-max-size", long = "docs-max-size")]
    pub max_size: usize,
}
