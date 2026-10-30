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

    /// Force the `Secure` flag on cookies (true) or never set it (false).
    /// Unset, cookies are secure only when `protocol` is https. Set it to true
    /// when Kellnr is reached over https through a TLS-terminating proxy but
    /// `protocol` is http
    pub cookie_secure: Option<bool>,
}

impl Default for Origin {
    fn default() -> Self {
        Self {
            hostname: default_hostname(),
            port: default_origin_port(),
            protocol: Protocol::Http,
            path: String::new(),
            cookie_secure: None,
        }
    }
}

impl Origin {
    /// Whether cookies must carry the `Secure` flag.
    pub fn secure_cookies(&self) -> bool {
        self.cookie_secure
            .unwrap_or(self.protocol == Protocol::Https)
    }

    /// Brings `path` into the form `/prefix`, or empty for the root: no
    /// surrounding whitespace, one leading slash and no trailing slash.
    /// Applied once when the settings are loaded, so callers can append
    /// `/...` to it.
    pub fn normalize(&mut self) {
        let path = self.path.trim().trim_matches('/');
        self.path = if path.is_empty() {
            String::new()
        } else {
            format!("/{path}")
        };
    }

    /// External URL of Kellnr without a trailing slash. The port is left out
    /// if it is the default port of the protocol.
    pub fn base_url(&self) -> String {
        let default_port = match self.protocol {
            Protocol::Http => 80,
            Protocol::Https => 443,
        };
        if self.port == default_port {
            format!("{}://{}{}", self.protocol, self.hostname, self.path)
        } else {
            format!(
                "{}://{}:{}{}",
                self.protocol, self.hostname, self.port, self.path
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
            cookie_secure: None,
        }
    }

    #[test]
    fn secure_cookies_follow_protocol_by_default() {
        let mut o = origin(443, "");
        assert!(o.secure_cookies());

        o.protocol = Protocol::Http;
        assert!(!o.secure_cookies());
    }

    /// Behind a TLS-terminating proxy the protocol may be http while
    /// browsers use https; operators can then force secure cookies.
    #[test]
    fn secure_cookies_can_be_overridden() {
        let mut o = origin(80, "");
        o.protocol = Protocol::Http;
        o.cookie_secure = Some(true);
        assert!(o.secure_cookies());

        let mut o = origin(443, "");
        o.cookie_secure = Some(false);
        assert!(!o.secure_cookies());
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

    #[test]
    fn base_url_keeps_port_that_is_not_the_protocol_default() {
        assert_eq!(origin(80, "").base_url(), "https://kellnr.example.com:80");

        let mut http = origin(443, "");
        http.protocol = Protocol::Http;
        assert_eq!(http.base_url(), "http://kellnr.example.com:443");
    }

    #[test]
    fn normalize_brings_path_into_prefix_form() {
        for (path, expected) in [
            ("", ""),
            ("/", ""),
            ("  /  ", ""),
            ("registry", "/registry"),
            ("/registry", "/registry"),
            ("/registry/", "/registry"),
            (" registry// ", "/registry"),
            ("/a/b/", "/a/b"),
        ] {
            let mut o = origin(443, path);
            o.normalize();
            assert_eq!(o.path, expected, "path {path:?}");
        }
    }

    /// A bare, missing or trailing slash must not leak into the URL, otherwise
    /// callers appending their own path produce a double slash.
    #[test]
    fn base_url_of_normalized_origin_composes_into_callback_url() {
        for path in ["", "/", "registry", "/registry", "/registry/"] {
            let mut o = origin(443, path);
            o.normalize();
            let url = format!("{}/api/v1/oauth2/callback", o.base_url());
            assert!(
                url == "https://kellnr.example.com/api/v1/oauth2/callback"
                    || url == "https://kellnr.example.com/registry/api/v1/oauth2/callback",
                "callback URL for path {path:?} is malformed: {url}"
            );
        }
    }
}
