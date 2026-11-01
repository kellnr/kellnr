use std::fmt;

use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs, ToSchema)]
#[serde(default)]
#[configurable(clap_prefix = "origin")]
pub struct Origin {
    /// External hostname for URLs
    pub hostname: String,

    /// External port for URLs
    pub port: u16,

    /// Protocol (http or https)
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

    /// The URL path below which Kellnr is served.
    pub fn path_prefix(&self) -> UrlPathPrefix {
        UrlPathPrefix::new(&self.path)
    }

    /// Brings `path` into the form of [`UrlPathPrefix`]. Applied once when
    /// the settings are loaded, so the settings show the path in use.
    pub fn normalize(&mut self) {
        self.path = self.path_prefix().0;
    }

    /// External URL of Kellnr without a trailing slash. The port is left out
    /// if it is the default port of the protocol.
    pub fn base_url(&self) -> String {
        let default_port = match self.protocol {
            Protocol::Http => 80,
            Protocol::Https => 443,
        };
        let path = self.path_prefix();
        if self.port == default_port {
            format!("{}://{}{path}", self.protocol, self.hostname)
        } else {
            format!("{}://{}:{}{path}", self.protocol, self.hostname, self.port)
        }
    }
}

/// URL path prefix of Kellnr: empty for the root, otherwise `/prefix` with
/// one leading and no trailing slash, so callers can append `/...` to it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UrlPathPrefix(String);

impl UrlPathPrefix {
    /// Accepts the path with or without leading and trailing slashes and
    /// surrounding whitespace.
    pub fn new(path: &str) -> Self {
        let path = path.trim().trim_matches('/');
        if path.is_empty() {
            Self::default()
        } else {
            Self(format!("/{path}"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether Kellnr is served at the root, without a prefix.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for UrlPathPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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
    fn path_prefix_has_one_leading_and_no_trailing_slash() {
        for (path, expected) in [
            ("", ""),
            ("/", ""),
            ("//", ""),
            ("  /  ", ""),
            ("registry", "/registry"),
            ("/registry", "/registry"),
            ("/registry/", "/registry"),
            (" registry// ", "/registry"),
            ("/a/b/", "/a/b"),
        ] {
            let prefix = UrlPathPrefix::new(path);
            assert_eq!(prefix.as_str(), expected, "path {path:?}");
            assert_eq!(prefix.to_string(), expected, "path {path:?}");
            assert_eq!(prefix.is_root(), expected.is_empty(), "path {path:?}");
            assert_eq!(origin(443, path).path_prefix(), prefix, "path {path:?}");
        }
    }

    #[test]
    fn base_url_normalizes_path_that_was_not_normalized() {
        assert_eq!(
            origin(443, "registry/").base_url(),
            "https://kellnr.example.com/registry"
        );
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
