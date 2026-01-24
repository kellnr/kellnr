use std::net::{IpAddr, Ipv4Addr};

use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

fn default_ip() -> IpAddr {
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct Local {
    /// IP address to bind to
    #[default(default_ip())]
    #[arg(id = "local-ip", long = "local-ip")]
    pub ip: IpAddr,

    /// Port to listen on
    #[default(8000)]
    #[arg(id = "local-port", long = "local-port", short = 'p')]
    pub port: u16,
}
