use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn default_admin_pwd() -> String {
    "admin".to_string()
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs, ToSchema)]
#[serde(default)]
#[configurable(clap_prefix = "setup")]
pub struct Setup {
    /// Initial admin password
    #[configurable(secret)]
    pub admin_pwd: String,

    /// Initial admin API token
    #[configurable(secret)]
    pub admin_token: Option<String>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            admin_pwd: default_admin_pwd(),
            admin_token: None,
        }
    }
}
