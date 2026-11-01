use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug, Default, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs, ToSchema,
)]
#[serde(default)]
#[configurable(clap_prefix = "webhooks")]
pub struct Webhooks {
    /// Refuse webhook deliveries to loopback, private and other non-public
    /// addresses. Link-local and cloud metadata addresses are always refused.
    pub block_private_targets: bool,
    /// Connect to webhook targets directly instead of through a proxy set in
    /// `HTTP_PROXY` or `HTTPS_PROXY`. Through a proxy, only the proxy's
    /// address can be checked against the blocked targets.
    pub ignore_env_proxy: bool,
}
