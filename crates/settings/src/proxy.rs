use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};
use url::Url;

fn default_proxy_url() -> Url {
    Url::parse("https://static.crates.io/crates/").unwrap()
}

fn default_index_url() -> Url {
    Url::parse("https://index.crates.io/").unwrap()
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct Proxy {
    /// Enable crates.io proxy
    #[default(false)]
    #[arg(id = "proxy-enabled", long = "proxy-enabled")]
    pub enabled: bool,

    /// Number of proxy threads
    #[default(10)]
    #[arg(id = "proxy-num-threads", long = "proxy-num-threads")]
    pub num_threads: usize,

    /// Download crates on index update
    #[default(false)]
    #[arg(id = "proxy-download-on-update", long = "proxy-download-on-update")]
    pub download_on_update: bool,

    /// Crates.io download URL
    #[default(default_proxy_url())]
    #[arg(id = "proxy-url", long = "proxy-url")]
    pub url: Url,

    /// Crates.io index URL
    #[default(default_index_url())]
    #[arg(id = "proxy-index", long = "proxy-index")]
    pub index: Url,
}
