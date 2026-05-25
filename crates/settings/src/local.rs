use std::net::{IpAddr, Ipv4Addr};

use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

fn default_ip() -> IpAddr {
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "local")]
pub struct Local {
    /// IP address to bind to
    pub ip: IpAddr,

    /// Port to listen on
    #[arg(short = 'p')]
    pub port: u16,
}

impl Default for Local {
    fn default() -> Self {
        Self {
            ip: default_ip(),
            port: 8000,
        }
    }
}
