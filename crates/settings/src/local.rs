use std::{net::IpAddr, str::FromStr};
use serde::{Serialize, Deserialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Local {
    pub ip: IpAddr,
    pub port: u16,
}

impl Default for Local {
    fn default() -> Self {
        Self {
            ip: IpAddr::from_str("0.0.0.0").unwrap(), // Unwrap is safe because the string is hardcoded
            port: 8000,
        }
    }
}
