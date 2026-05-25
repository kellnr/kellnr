use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};
use url::Url;

fn default_proxy_url() -> Url {
    Url::parse("https://static.crates.io/crates/").unwrap()
}

fn default_index_url() -> Url {
    Url::parse("https://index.crates.io/").unwrap()
}

fn default_api_url() -> Url {
    Url::parse("https://crates.io/api/v1/crates/").unwrap()
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "proxy")]
pub struct Proxy {
    /// Enable crates.io proxy
    pub enabled: bool,

    /// Number of proxy threads
    pub num_threads: usize,

    /// Download crates on index update
    pub download_on_update: bool,

    /// Crates.io download URL
    pub url: Url,

    /// Crates.io index URL
    pub index: Url,

    /// Crates.io API URL
    pub api: Url,

    /// Connect timeout in seconds for upstream requests
    #[arg(long = "proxy-connect-timeout")]
    pub connect_timeout_seconds: u64,

    /// Request timeout in seconds for upstream downloads
    #[arg(long = "proxy-request-timeout")]
    pub request_timeout_seconds: u64,
}

impl Default for Proxy {
    fn default() -> Self {
        Self {
            enabled: false,
            num_threads: 10,
            download_on_update: false,
            url: default_proxy_url(),
            index: default_index_url(),
            api: default_api_url(),
            connect_timeout_seconds: 5,
            request_timeout_seconds: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_api_url_is_cratesio() {
        let proxy = Proxy::default();
        assert_eq!(
            proxy.api,
            Url::parse("https://crates.io/api/v1/crates/").unwrap()
        );
    }

    #[test]
    fn deserialize_custom_api_url_from_toml() {
        let toml = r#"
            api = "https://rsproxy.cn/api/v1/crates/"
        "#;
        let proxy: Proxy = toml::from_str(toml).unwrap();
        assert_eq!(
            proxy.api,
            Url::parse("https://rsproxy.cn/api/v1/crates/").unwrap()
        );
    }

    #[test]
    fn missing_api_url_uses_default() {
        let toml = r#"
            enabled = true
            url = "https://rsproxy.cn/api/v1/crates/"
            index = "https://rsproxy.cn/index/"
        "#;
        let proxy: Proxy = toml::from_str(toml).unwrap();
        assert_eq!(
            proxy.api,
            Url::parse("https://crates.io/api/v1/crates/").unwrap()
        );
    }

    #[test]
    fn api_url_join_produces_correct_crate_url() {
        let proxy: Proxy = toml::from_str(r#"api = "https://rsproxy.cn/api/v1/crates/""#).unwrap();
        let url = proxy.api.join("serde").unwrap();
        assert_eq!(url.as_str(), "https://rsproxy.cn/api/v1/crates/serde");
    }

    #[test]
    fn default_url_join_produces_correct_crate_url() {
        let proxy = Proxy::default();
        let url = proxy.api.join("tokio").unwrap();
        assert_eq!(url.as_str(), "https://crates.io/api/v1/crates/tokio");
    }
}
