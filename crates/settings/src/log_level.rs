use crate::deserialize_with::DeserializeWith;
use serde::{Deserialize, Deserializer};
use tracing::Level;

impl DeserializeWith for Level {
    fn deserialize_with<'de, D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(de)?.to_lowercase();

        match s.as_ref() {
            "trace" => Ok(Level::TRACE),
            "debug" => Ok(Level::DEBUG),
            "info" => Ok(Level::INFO),
            "warn" => Ok(Level::WARN),
            "error" => Ok(Level::ERROR),
            _ => Err(serde::de::Error::custom(
                "error trying to deserialize log level config",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Settings {
        #[serde(deserialize_with = "Level::deserialize_with")]
        log_level: Level,
    }

    #[test]
    fn test_deserialize_log_level_trace() {
        let toml = r#"
            log_level = "trace"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, Level::TRACE);
    }

    #[test]
    fn test_deserialize_log_level_debug() {
        let toml = r#"
            log_level = "debug"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, Level::DEBUG);
    }

    #[test]
    fn test_deserialize_log_level_info() {
        let toml = r#"
            log_level = "info"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, Level::INFO);
    }

    #[test]
    fn test_deserialize_log_level_warn() {
        let toml = r#"
            log_level = "warn"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, Level::WARN);
    }

    #[test]
    fn test_deserialize_log_level_error() {
        let toml = r#"
            log_level = "error"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, Level::ERROR);
    }

    #[test]
    fn test_deserialize_log_level_uppercase() {
        let toml = r#"
        log_level = "DEBUG"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, Level::DEBUG);
    }

    #[test]
    fn test_deserialize_log_level_invalid() {
        let toml = r#"
        log_level = "no_log_level"
        "#;

        let settings: Result<Settings, toml::de::Error> = toml::from_str(toml);
        assert!(settings.is_err());
    }
}
