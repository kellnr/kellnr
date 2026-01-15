use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
#[serde(default)]
pub struct Proxy {
    pub enabled: bool,
    pub num_threads: usize,
    pub download_on_update: bool,
    pub url: Url,
    pub index: Url,
}

impl Default for Proxy {
    fn default() -> Self {
        Self {
            enabled: false,
            num_threads: 10,
            download_on_update: false,
            url: Url::parse("https://static.crates.io/crates/").unwrap(),
            index: Url::parse("https://index.crates.io/").unwrap(),
        }
    }
}
