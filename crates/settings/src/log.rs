use crate::deserialize_with::DeserializeWith;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::Display;

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Log {
    #[serde(deserialize_with = "LogFormat::deserialize_with")]
    pub format: LogFormat,
    #[serde(deserialize_with = "LogLevel::deserialize_with")]
    pub level: LogLevel,
    #[serde(deserialize_with = "LogLevel::deserialize_with")]
    pub level_web_server: LogLevel,
}

impl Default for Log {
    fn default() -> Self {
        Self {
            format: LogFormat::Compact,
            level: LogLevel::Info,
            level_web_server: LogLevel::Warn,
        }
    }
}

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
                "error trying to deserialize log format: {s}",
            )),
        }
    }
}

impl Serialize for LogFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

impl Serialize for LogLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl DeserializeWith for LogLevel {
    fn deserialize_with<'de, D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(de)?.to_lowercase();

        match s.as_ref() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(serde::de::Error::custom(
                "error trying to deserialize log level: {s}",
            )),
        }
    }
}

impl From<LogLevel> for tracing::Level {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}

impl From<LogLevel> for tracing::level_filters::LevelFilter {
    fn from(value: LogLevel) -> Self {
        Self::from_level(value.into())
    }
}
 
#[cfg(test)]
mod log_format_tests {
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

#[cfg(test)]
mod log_level_tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Settings {
        #[serde(deserialize_with = "LogLevel::deserialize_with")]
        log_level: LogLevel,
    }

    #[test]
    fn test_deserialize_log_level_trace() {
        let toml = r#"
            log_level = "trace"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, LogLevel::Trace);
    }

    #[test]
    fn test_deserialize_log_level_debug() {
        let toml = r#"
            log_level = "debug"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, LogLevel::Debug);
    }

    #[test]
    fn test_deserialize_log_level_info() {
        let toml = r#"
            log_level = "info"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, LogLevel::Info);
    }

    #[test]
    fn test_deserialize_log_level_warn() {
        let toml = r#"
            log_level = "warn"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, LogLevel::Warn);
    }

    #[test]
    fn test_deserialize_log_level_error() {
        let toml = r#"
            log_level = "error"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, LogLevel::Error);
    }

    #[test]
    fn test_deserialize_log_level_uppercase() {
        let toml = r#"
        log_level = "DEBUG"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log_level, LogLevel::Debug);
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

#[cfg(test)]
mod log_tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Settings {
       pub log: Log,
    }

    #[test]
    fn test_deserialize_whole_log() {
        let toml = r#"
        [no_log]
        foo = "bar"

        [log]
        level = "trace"
        level_web_server = "debug"
        format = "compact"
        "#;

        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.log.level, LogLevel::Trace);
        assert_eq!(settings.log.level_web_server, LogLevel::Debug);
        assert_eq!(settings.log.format, LogFormat::Compact);
    }
}
