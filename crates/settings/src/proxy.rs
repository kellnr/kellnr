use kellnr_common::base_url::{BaseUrl, BaseUrlError};
use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;

fn default_proxy_url() -> Url {
    Url::parse("https://static.crates.io/crates/").unwrap()
}

fn default_index_url() -> Url {
    Url::parse("https://index.crates.io/").unwrap()
}

fn default_api_url() -> Url {
    Url::parse("https://crates.io/api/v1/crates/").unwrap()
}

/// Default user-agent sent with requests to crates.io. This is the single source
/// of truth for the default user-agent across the whole codebase.
pub const DEFAULT_USER_AGENT: &str = "kellnr.io/kellnr";

fn default_user_agent() -> String {
    DEFAULT_USER_AGENT.to_string()
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs, ToSchema)]
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
    #[schema(value_type = String)]
    pub url: Url,

    /// Crates.io index URL
    #[schema(value_type = String)]
    pub index: Url,

    /// Crates.io API URL
    #[schema(value_type = String)]
    pub api: Url,

    /// Connect timeout in seconds for upstream requests
    #[arg(long = "proxy-connect-timeout")]
    pub connect_timeout_seconds: u64,

    /// Request timeout in seconds for upstream downloads
    #[arg(long = "proxy-request-timeout")]
    pub request_timeout_seconds: u64,

    /// User-agent sent with requests to crates.io. crates.io's data-access
    /// policy asks for a user-agent that identifies the application and a way
    /// to get in contact, e.g. "kellnr.io/kellnr (contact@example.com)".
    #[arg(long = "proxy-user-agent")]
    pub user_agent: String,
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
            user_agent: default_user_agent(),
        }
    }
}

impl Proxy {
    /// Lets the upstream URLs end in a slash. They are base URLs for
    /// `Url::join`, which replaces the last path segment if it is missing.
    pub fn normalize(&mut self) {
        for url in [&mut self.url, &mut self.index, &mut self.api] {
            if !url.path().ends_with('/') {
                let path = format!("{}/", url.path());
                url.set_path(&path);
            }
        }
    }

    /// Base URL of the crate downloads, `proxy.url`.
    pub fn download_url(&self) -> Result<BaseUrl, BaseUrlError> {
        BaseUrl::new(self.url.clone())
    }

    /// Base URL of the sparse index, `proxy.index`.
    pub fn index_url(&self) -> Result<BaseUrl, BaseUrlError> {
        BaseUrl::new(self.index.clone())
    }

    /// Base URL of the crates API, `proxy.api`.
    pub fn api_url(&self) -> Result<BaseUrl, BaseUrlError> {
        BaseUrl::new(self.api.clone())
    }

    /// Checks the upstream URLs of an enabled proxy, so the accessors of
    /// them cannot fail later.
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        for (key, url) in [
            ("proxy.url", self.download_url()),
            ("proxy.index", self.index_url()),
            ("proxy.api", self.api_url()),
        ] {
            url.map_err(|e| format!("{key} {e}"))?;
        }
        Ok(())
    }

    /// Configuration that is accepted but most likely a mistake.
    pub fn warnings(&self) -> Vec<String> {
        if self.enabled && self.num_threads == 0 {
            vec![
                "proxy.num_threads is 0, cached crates.io crates are not updated in the background"
                    .to_string(),
            ]
        } else {
            Vec::new()
        }
    }

    /// The upstream sparse-index URL that internal cargo invocations (e.g. the
    /// docs builder) should resolve crates.io dependencies from, if it differs
    /// from the built-in crates.io default.
    ///
    /// Returns `None` when the proxy is disabled or the default index is in
    /// use. In both cases cargo's own default (`index.crates.io`) is correct
    /// and source replacement would be redundant. When `Some`, callers should
    /// point cargo's `crates-io` source at this URL so dependency resolution
    /// honors `proxy.index` instead of hitting crates.io directly. See #1185.
    pub fn cratesio_index_override(&self) -> Option<&Url> {
        if self.enabled && self.index != default_index_url() {
            Some(&self.index)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lets_urls_end_in_a_slash() {
        let mut proxy: Proxy = toml::from_str(
            r#"
            url = "https://mirror.example.com/crates"
            index = "https://mirror.example.com/index"
            api = "https://mirror.example.com/api/v1/crates/"
        "#,
        )
        .unwrap();

        proxy.normalize();

        assert_eq!(
            proxy.url.join("serde/serde-1.0.0.crate").unwrap().as_str(),
            "https://mirror.example.com/crates/serde/serde-1.0.0.crate"
        );
        assert_eq!(proxy.index.as_str(), "https://mirror.example.com/index/");
        assert_eq!(
            proxy.api.as_str(),
            "https://mirror.example.com/api/v1/crates/"
        );
    }

    #[test]
    fn accessors_return_base_urls_that_keep_the_last_segment() {
        let proxy: Proxy = toml::from_str(
            r#"
            url = "https://mirror.example.com/crates"
            index = "https://mirror.example.com/index"
            api = "https://mirror.example.com/api/v1/crates"
        "#,
        )
        .unwrap();

        assert_eq!(
            proxy
                .download_url()
                .unwrap()
                .join("serde/1.0.0/download")
                .as_str(),
            "https://mirror.example.com/crates/serde/1.0.0/download"
        );
        assert_eq!(
            proxy.index_url().unwrap().join("se/rd/serde").as_str(),
            "https://mirror.example.com/index/se/rd/serde"
        );
        assert_eq!(
            proxy.api_url().unwrap().join("serde").as_str(),
            "https://mirror.example.com/api/v1/crates/serde"
        );
    }

    #[test]
    fn validate_names_the_key_of_an_invalid_url() {
        let proxy = Proxy {
            enabled: true,
            index: Url::parse("sparse+https://index.crates.io/").unwrap(),
            ..Proxy::default()
        };
        assert_eq!(
            proxy.validate().unwrap_err(),
            "proxy.index must be an http or https URL, got sparse+https://index.crates.io/"
        );
    }

    #[test]
    fn zero_threads_are_accepted_with_a_warning() {
        let proxy = Proxy {
            enabled: true,
            num_threads: 0,
            ..Proxy::default()
        };
        assert!(proxy.validate().is_ok());
        assert!(proxy.warnings()[0].contains("num_threads"));
        assert!(Proxy::default().warnings().is_empty());
    }

    #[test]
    fn validate_rejects_urls_that_are_not_http() {
        let proxy = Proxy {
            enabled: true,
            index: Url::parse("sparse+https://index.crates.io/").unwrap(),
            ..Proxy::default()
        };
        assert!(proxy.validate().unwrap_err().contains("proxy.index"));
    }

    #[test]
    fn validate_ignores_disabled_proxy() {
        let proxy = Proxy {
            num_threads: 0,
            url: Url::parse("ftp://example.com/").unwrap(),
            ..Proxy::default()
        };
        assert!(proxy.validate().is_ok());
        let enabled = Proxy {
            enabled: true,
            ..Proxy::default()
        };
        assert!(enabled.validate().is_ok());
    }

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

    #[test]
    fn default_user_agent_is_kellnr() {
        let proxy = Proxy::default();
        assert_eq!(proxy.user_agent, "kellnr.io/kellnr");
    }

    #[test]
    fn missing_user_agent_uses_default() {
        let proxy: Proxy = toml::from_str("enabled = true").unwrap();
        assert_eq!(proxy.user_agent, "kellnr.io/kellnr");
    }

    #[test]
    fn deserialize_custom_user_agent_from_toml() {
        let toml = r#"user_agent = "kellnr.io/kellnr (admin@example.com)""#;
        let proxy: Proxy = toml::from_str(toml).unwrap();
        assert_eq!(proxy.user_agent, "kellnr.io/kellnr (admin@example.com)");
    }

    #[test]
    fn index_override_none_when_disabled() {
        let proxy: Proxy = toml::from_str(r#"index = "https://rsproxy.cn/index/""#).unwrap();
        assert!(!proxy.enabled);
        assert_eq!(proxy.cratesio_index_override(), None);
    }

    #[test]
    fn index_override_none_when_default_index() {
        let proxy: Proxy = toml::from_str("enabled = true").unwrap();
        assert_eq!(proxy.cratesio_index_override(), None);
    }

    #[test]
    fn index_override_some_when_enabled_and_custom() {
        let toml = r#"
            enabled = true
            index = "https://rsproxy.cn/index/"
        "#;
        let proxy: Proxy = toml::from_str(toml).unwrap();
        assert_eq!(
            proxy.cratesio_index_override().map(Url::as_str),
            Some("https://rsproxy.cn/index/")
        );
    }
}
