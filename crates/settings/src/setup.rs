use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

fn default_admin_pwd() -> String {
    "admin".to_string()
}

fn default_admin_token() -> String {
    "Zy9HhJ02RJmg0GCrgLfaCVfU6IwDfhXD".to_string()
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct Setup {
    /// Initial admin password
    #[default(default_admin_pwd())]
    #[arg(id = "setup-admin-pwd", long = "setup-admin-pwd")]
    pub admin_pwd: String,

    /// Initial admin API token
    #[default(default_admin_token())]
    #[arg(id = "setup-admin-token", long = "setup-admin-token")]
    pub admin_token: String,
}
