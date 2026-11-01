use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs, ToSchema)]
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

impl Toolchain {
    /// `max_size` in bytes.
    pub fn max_bytes(&self) -> usize {
        crate::registry::mb_to_bytes(self.max_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_bytes_saturates() {
        let mut toolchain = Toolchain::default();
        assert_eq!(toolchain.max_bytes(), 500_000_000);
        toolchain.max_size = usize::MAX;
        assert_eq!(toolchain.max_bytes(), usize::MAX);
    }
}
