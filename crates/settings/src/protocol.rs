use crate::deserialize_with::DeserializeWith;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http,
    Https,
}

impl Default for Protocol {
    fn default() -> Self {
        Self::Http
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Http => write!(f, "http"),
            Protocol::Https => write!(f, "https"),
        }
    }
}

impl<'de> Deserialize<'de> for Protocol {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Protocol::deserialize_with(de)
    }
}

impl DeserializeWith for Protocol {
    fn deserialize_with<'de, D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(de)?.to_lowercase();

        match s.as_ref() {
            "http" => Ok(Protocol::Http),
            "https" => Ok(Protocol::Https),
            _ => Err(serde::de::Error::custom(
                "error trying to deserialize protocol config",
            )),
        }
    }
}

impl Serialize for Protocol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Protocol::Http => serializer.serialize_str("http"),
            Protocol::Https => serializer.serialize_str("https"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn test_protocol_display() {
        assert_eq!(Protocol::Http.to_string(), "http");
        assert_eq!(Protocol::Https.to_string(), "https");
    }

    #[derive(Debug, Deserialize)]
    struct Settings {
        #[serde(deserialize_with = "Protocol::deserialize_with")]
        protocol: Protocol,
    }

    #[test]
    fn test_deserialize_protocol_https() {
        let toml = r#"
            protocol = "https"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.protocol, Protocol::Https);
    }

    #[test]
    fn test_deserialize_protocol_http() {
        let toml = r#"
            protocol = "http"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.protocol, Protocol::Http);
    }

    #[test]
    fn test_deserialize_protocol_uppercase() {
        let toml = r#"
            protocol = "HTTPS"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.protocol, Protocol::Https);
    }

    #[test]
    fn test_deserialize_protocol_error() {
        let toml = r#"
            protocol = "ftp"
        "#;

        let settings: Result<Settings, toml::de::Error> = toml::from_str(toml);
        assert!(settings.is_err());
    }

    #[test]
    fn test_serialize_protocol_http() {
        let protocol = Protocol::Http;
        let json = serde_json::to_string(&protocol).unwrap();
        assert_eq!(json, r#""http""#);
    }

    #[test]
    fn test_serialize_protocol_https() {
        let protocol = Protocol::Https;
        let json = serde_json::to_string(&protocol).unwrap();
        assert_eq!(json, r#""https""#);
    }
}
