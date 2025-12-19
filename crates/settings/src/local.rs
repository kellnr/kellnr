use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
#[serde(default)]
pub struct Local {
    pub ip: IpAddr,
    pub port: u16,
}

impl Default for Local {
    fn default() -> Self {
        Self {
            ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: 8000,
        }
    }
}
