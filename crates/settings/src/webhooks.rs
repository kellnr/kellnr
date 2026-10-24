use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "webhooks")]
pub struct Webhooks {
    /// Refuse webhook deliveries to loopback, private and other non-public
    /// addresses. Link-local and cloud metadata addresses are always refused.
    pub block_private_targets: bool,
}
