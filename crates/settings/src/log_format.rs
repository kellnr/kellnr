use crate::deserialize_with::DeserializeWith;
use serde::{Deserialize, Deserializer};
use std::fmt::Display;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LogFormat {
    Compact,
    Pretty,
    Json,
}

impl DeserializeWith for LogFormat {
    fn deserialize_with<'de, D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(de)?.to_lowercase();

        match s.as_ref() {
            "compact" => Ok(LogFormat::Compact),
            "pretty" => Ok(LogFormat::Pretty),
            "json" => Ok(LogFormat::Json),
            _ => Err(serde::de::Error::custom(
                "error trying to deserialize log level config",
            )),
        }
    }
}

impl Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogFormat::Compact => write!(f, "compact"),
            LogFormat::Pretty => write!(f, "pretty"),
            LogFormat::Json => write!(f, "json"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Settings {
        #[serde(deserialize_with = "LogFormat::deserialize_with")]
        log_format: LogFormat,
    }

    #[test]
    fn test_deserialize_log_format_compact() {
        let toml = r#"
            log_format = "compact"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_format, LogFormat::Compact);
    }

    #[test]
    fn test_deserialize_log_format_pretty() {
        let toml = r#"
            log_format = "pretty"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_format, LogFormat::Pretty);
    }

    #[test]
    fn test_deserialize_log_format_json() {
        let toml = r#"
            log_format = "json"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_format, LogFormat::Json);
    }

    #[test]
    fn test_deserialize_log_format_invalid() {
        let toml = r#"
        log_level = "no_log_format"
        "#;

        let settings: Result<Settings, toml::de::Error> = toml::from_str(toml);
        assert!(settings.is_err());
    }
}
