use clap_serde_derive::ClapSerde;
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

    /// Crates.io API URL
    #[default(default_api_url())]
    #[arg(id = "proxy-api", long = "proxy-api")]
    pub api: Url,

    /// Connect timeout in seconds for upstream requests
    #[default(5)]
    #[arg(id = "proxy-connect-timeout", long = "proxy-connect-timeout")]
    pub connect_timeout_seconds: u64,

    /// Request timeout in seconds for upstream downloads
    #[default(30)]
    #[arg(id = "proxy-request-timeout", long = "proxy-request-timeout")]
    pub request_timeout_seconds: u64,
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
