use kellnr_settings::{Protocol, Settings};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
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

    pub fn new(
        protocol: Protocol,
        api_address: &str,
        api_port: u16,
        api_url_path: Option<&str>,
        api_path: &str,
        api_available: bool,
        auth_required: bool,
    ) -> Self {
        let url = format!("{protocol}://{api_address}:{api_port}");
        let url_end = format!("/api/v1/{api_path}/dl");
        let url = match api_url_path {
            Some(api_url_path) => {
                let api_url_path = api_url_path.trim_matches('/');
                if !api_url_path.is_empty() {
                    format!("{url}/{api_url_path}")
                } else {
                    url
                }
            }
            None => url,
        };
        Self {
            dl: format!("{url}{url_end}"),
            api: if api_available { Some(url) } else { None },
            auth_required,
        }
    }
}

impl From<(&Settings, &str, bool)> for ConfigJson {
    fn from(value: (&Settings, &str, bool)) -> Self {
        let api_url_path: Option<&str> = if value.0.origin.path.is_empty() {
            None
        } else {
            Some(&value.0.origin.path)
        };
        Self::new(
            value.0.origin.protocol,
            &value.0.origin.hostname,
            value.0.origin.port,
            api_url_path,
            value.1,
            value.2,
            value.0.registry.auth_required,
        )
    }
}

#[cfg(test)]
mod tests {
    use kellnr_settings::Protocol;

    use super::*;

    #[test]
    fn test_config_json_to_json_http() {
        let config = ConfigJson::new(Protocol::Http, "localhost", 8080, None, "path", true, false);
        let json = config.to_json().unwrap();

        assert_eq!(
            json,
            r#"{"dl":"http://localhost:8080/api/v1/path/dl","api":"http://localhost:8080","auth-required":false}"#
        );
    }

    #[test]
    fn test_config_json_to_json_http_with_url_path() {
        let config = ConfigJson::new(
            Protocol::Http,
            "localhost",
            8080,
            Some("/kellnring/"),
            "path",
            true,
            false,
        );
        let json = config.to_json().unwrap();

        assert_eq!(
            json,
            r#"{"dl":"http://localhost:8080/kellnring/api/v1/path/dl","api":"http://localhost:8080/kellnring","auth-required":false}"#
        );
    }

    #[test]
    fn test_config_json_to_json_https() {
        let config = ConfigJson::new(Protocol::Https, "localhost", 8081, None, "path", true, true);
        let json = config.to_json().unwrap();

        assert_eq!(
            json,
            r#"{"dl":"https://localhost:8081/api/v1/path/dl","api":"https://localhost:8081","auth-required":true}"#
        );
    }

    #[test]
    fn test_config_json_no_api() {
        let config = ConfigJson::new(
            Protocol::Https,
            "localhost",
            8081,
            None,
            "path",
            false,
            true,
        );
        let json = config.to_json().unwrap();

        assert_eq!(
            json,
            r#"{"dl":"https://localhost:8081/api/v1/path/dl","auth-required":true}"#
        );
    }
}
