use kellnr_settings::Settings;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The two registries Kellnr serves an index for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKind {
    /// The crates published to Kellnr.
    Kellnr,
    /// The crates.io proxy, which only serves downloads.
    CratesIo,
}

impl IndexKind {
    /// Path segment of the API below `/api/v1`.
    fn api_path(self) -> &'static str {
        match self {
            Self::Kellnr => "crates",
            Self::CratesIo => "cratesio",
        }
    }

    /// Whether cargo may use the web API of the registry, e.g. to publish.
    fn has_api(self) -> bool {
        match self {
            Self::Kellnr => true,
            Self::CratesIo => false,
        }
    }
}

/// The `config.json` of a sparse registry index, see
/// <https://doc.rust-lang.org/cargo/reference/registry-index.html#index-configuration>.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, ToSchema)]
pub struct ConfigJson {
    dl: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    api: Option<String>,
    #[serde(rename = "auth-required")]
    auth_required: bool,
}

impl ConfigJson {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }

    /// The `config.json` of the index of `kind`.
    pub fn new(settings: &Settings, kind: IndexKind) -> Self {
        let origin = &settings.origin;
        let url = format!(
            "{}://{}:{}{}",
            origin.protocol,
            origin.hostname,
            origin.port,
            origin.path_prefix()
        );
        Self {
            dl: format!("{url}/api/v1/{}/dl", kind.api_path()),
            api: kind.has_api().then_some(url),
            auth_required: settings.registry.auth_required,
        }
    }
}

#[cfg(test)]
mod tests {
    use kellnr_settings::{Origin, Protocol};

    use super::*;

    fn settings(protocol: Protocol, port: u16, path: &str, auth_required: bool) -> Settings {
        let mut settings = kellnr_settings::test_settings();
        settings.origin = Origin {
            hostname: "localhost".to_string(),
            port,
            protocol,
            path: path.to_string(),
            cookie_secure: None,
        };
        settings.registry.auth_required = auth_required;
        settings
    }

    fn json(settings: &Settings, kind: IndexKind) -> String {
        ConfigJson::new(settings, kind).to_json().unwrap()
    }

    #[test]
    fn kellnr_config_json_http() {
        assert_eq!(
            json(
                &settings(Protocol::Http, 8080, "", false),
                IndexKind::Kellnr
            ),
            r#"{"dl":"http://localhost:8080/api/v1/crates/dl","api":"http://localhost:8080","auth-required":false}"#
        );
    }

    #[test]
    fn kellnr_config_json_with_url_path() {
        for path in ["/kellnring/", "kellnring", "/kellnring"] {
            assert_eq!(
                json(
                    &settings(Protocol::Http, 8080, path, false),
                    IndexKind::Kellnr
                ),
                r#"{"dl":"http://localhost:8080/kellnring/api/v1/crates/dl","api":"http://localhost:8080/kellnring","auth-required":false}"#,
                "path {path:?}"
            );
        }
    }

    #[test]
    fn kellnr_config_json_https_keeps_default_port() {
        assert_eq!(
            json(&settings(Protocol::Https, 443, "", true), IndexKind::Kellnr),
            r#"{"dl":"https://localhost:443/api/v1/crates/dl","api":"https://localhost:443","auth-required":true}"#
        );
    }

    #[test]
    fn cratesio_config_json_has_no_api() {
        assert_eq!(
            json(
                &settings(Protocol::Https, 8081, "", true),
                IndexKind::CratesIo
            ),
            r#"{"dl":"https://localhost:8081/api/v1/cratesio/dl","auth-required":true}"#
        );
        assert_eq!(
            json(
                &settings(Protocol::Http, 8080, "/kellnr", false),
                IndexKind::CratesIo
            ),
            r#"{"dl":"http://localhost:8080/kellnr/api/v1/cratesio/dl","auth-required":false}"#
        );
    }
}
