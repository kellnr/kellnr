use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use settings::{Protocol, Settings};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ConfigJson {
    dl: String,
    api: String,
    #[serde(rename = "auth-required")]
    auth_required: bool,
}

impl ConfigJson {
    pub fn to_json(&self) -> Result<String> {
        let json = serde_json::to_string(&self).with_context(|| "Unable to write config.json")?;
        Ok(json)
    }

    pub fn new(
        protocol: &Protocol,
        api_address: &str,
        api_port: u16,
        api_path: &str,
        auth_required: bool,
    ) -> Self {
        Self {
            dl: format!(
                "{}://{}:{}/api/v1/{}",
                protocol, api_address, api_port, api_path
            ),
            api: format!("{}://{}:{}", protocol, api_address, api_port),
            auth_required,
        }
    }
}

impl From<(&Settings, &str)> for ConfigJson {
    fn from(value: (&Settings, &str)) -> Self {
        Self::new(
            &value.0.origin.protocol,
            &value.0.origin.hostname,
            value.0.origin.port,
            value.1,
            value.0.registry.auth_required,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use settings::Protocol;

    #[test]
    fn test_config_json_to_json_http() {
        let config = ConfigJson::new(&Protocol::Http, "localhost", 8080, "path", false);
        let json = config.to_json().unwrap();

        assert_eq!(
            json,
            r#"{"dl":"http://localhost:8080/api/v1/path","api":"http://localhost:8080","auth-required":false}"#
        );
    }

    #[test]
    fn test_config_json_to_json_https() {
        let config = ConfigJson::new(&Protocol::Https, "localhost", 8081, "path", true);
        let json = config.to_json().unwrap();

        assert_eq!(
            json,
            r#"{"dl":"https://localhost:8081/api/v1/path","api":"https://localhost:8081","auth-required":true}"#
        );
    }
}
