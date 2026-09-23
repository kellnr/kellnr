use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

fn default_hostname() -> String {
    "127.0.0.1".to_string()
}

fn default_origin_port() -> u16 {
    std::env::var("KELLNR_ORIGIN__PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8000)
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "origin")]
pub struct Origin {
    /// External hostname for URLs
    pub hostname: String,

    /// External port for URLs
    pub port: u16,

    /// Protocol (http or https), not exposed on the CLI; set via TOML/env
    #[arg(skip)]
    pub protocol: Protocol,

    /// URL path prefix
    pub path: String,
}

impl Default for Origin {
    fn default() -> Self {
        Self {
            hostname: default_hostname(),
            port: default_origin_port(),
            protocol: Protocol::Http,
            path: String::new(),
        }
    }
}

impl Origin {
    pub fn base_url(&self) -> String {
        let path_prefix = self.path.trim().trim_end_matches('/');
        if self.port == 443 || self.port == 80 {
            format!("{}://{}{}", self.protocol, self.hostname, path_prefix)
        } else {
            format!(
                "{}://{}:{}{}",
                self.protocol, self.hostname, self.port, path_prefix
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(port: u16, path: &str) -> Origin {
        Origin {
            hostname: "kellnr.example.com".to_string(),
            port,
            protocol: Protocol::Https,
            path: path.to_string(),
        }
    }

    #[test]
    fn base_url_omits_default_ports() {
        assert_eq!(
            origin(443, "").base_url(),
            "https://kellnr.example.com",
            "443 is implied by https and must not be spelled out"
        );

        let mut http = origin(80, "");
        http.protocol = Protocol::Http;
        assert_eq!(http.base_url(), "http://kellnr.example.com");
    }

    #[test]
    fn base_url_includes_non_default_port() {
        assert_eq!(
            origin(8000, "").base_url(),
            "https://kellnr.example.com:8000"
        );
    }

    #[test]
    fn base_url_appends_path_prefix() {
        assert_eq!(
            origin(8000, "/registry").base_url(),
            "https://kellnr.example.com:8000/registry"
        );
        assert_eq!(
            origin(443, "/registry").base_url(),
            "https://kellnr.example.com/registry"
        );
    }

    /// A bare or trailing slash must not leak into the URL, otherwise callers
    /// appending their own path produce a double slash.
    #[test]
    fn base_url_normalizes_slashes_in_path_prefix() {
        for path in ["", "/", "  /  ", "/registry/", "/registry//"] {
            let url = origin(8000, path).base_url();
            assert!(
                !url.ends_with('/'),
                "base_url for path {path:?} must not end in a slash, got {url}"
            );
        }

        assert_eq!(
            origin(8000, "/registry/").base_url(),
            "https://kellnr.example.com:8000/registry"
        );
    }

    /// The `OAuth2` callback and post-logout redirect are both built by appending
    /// to `base_url`, so the join must stay free of double slashes.
    #[test]
    fn base_url_composes_into_callback_url() {
        for path in ["", "/", "/registry", "/registry/"] {
            let url = format!("{}/api/v1/oauth2/callback", origin(443, path).base_url());
            assert!(
                !url.contains("com//") && !url.contains("registry//"),
                "callback URL for path {path:?} has a double slash: {url}"
            );
        }
    }
}
