//! Config printing for `kellnr config show` command.
//!
//! Uses serde serialization to automatically enumerate all settings fields,
//! ensuring new fields are never forgotten in the config output.

use std::fmt::Write;

use kellnr_settings::{ConfigSource, Settings, ShowConfigOptions};
use toml::Value;

fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Value::Integer(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else {
                let items: Vec<String> = arr.iter().map(format_value).collect();
                format!("[{}]", items.join(", "))
            }
        }
        Value::Table(_) => "{ ... }".to_string(), // Nested tables shouldn't appear at field level
        Value::Datetime(dt) => dt.to_string(),
    }
}

struct ConfigPrinter<'a> {
    settings: &'a Settings,
    options: &'a ShowConfigOptions,
}

impl<'a> ConfigPrinter<'a> {
    fn new(settings: &'a Settings, options: &'a ShowConfigOptions) -> Self {
        Self { settings, options }
    }

    fn source_comment(&self, key: &str) -> &'static str {
        if self.options.show_sources {
            // Only show source comment for non-default values
            match self.settings.sources.get(key) {
                Some(ConfigSource::Toml) => " # source: toml",
                Some(ConfigSource::Env) => " # source: env",
                Some(ConfigSource::Cli) => " # source: cli",
                Some(ConfigSource::Default) | None => "",
            }
        } else {
            ""
        }
    }

    fn should_show(&self, key: &str) -> bool {
        if !self.options.no_defaults {
            return true;
        }
        matches!(
            self.settings.sources.get(key),
            Some(ConfigSource::Toml | ConfigSource::Env | ConfigSource::Cli)
        )
    }

    fn write_section(
        &self,
        output: &mut String,
        section_name: &str,
        section_table: &toml::map::Map<String, Value>,
        default_table: &toml::map::Map<String, Value>,
    ) {
        let mut section_output = String::new();

        for (field_name, field_value) in section_table {
            let key = format!("{section_name}.{field_name}");

            // For setup section, skip fields that match defaults (sensitive data)
            if section_name == "setup"
                && let Some(default_field) = default_table.get(field_name)
                && field_value == default_field
            {
                continue;
            }

            if !self.should_show(&key) {
                continue;
            }

            let formatted = format_value(field_value);
            let comment = self.source_comment(&key);
            let _ = writeln!(section_output, "{field_name} = {formatted}{comment}");
        }

        if !section_output.is_empty() {
            let _ = writeln!(output, "[{section_name}]");
            output.push_str(&section_output);
            output.push('\n');
        }
    }

    /// Print all settings sections by iterating over the serialized Settings struct.
    ///
    /// New sections added to Settings are automatically included without manual updates.
    fn print(&self) {
        let mut output = String::new();
        let defaults = Settings::default();

        // Serialize both to TOML tables for automatic iteration
        let Value::Table(settings_table) =
            Value::try_from(self.settings).expect("Settings should serialize to TOML")
        else {
            return;
        };
        let Value::Table(defaults_table) =
            Value::try_from(&defaults).expect("Default settings should serialize to TOML")
        else {
            return;
        };

        // Iterate over all sections automatically
        for (section_name, section_value) in &settings_table {
            let Value::Table(section_table) = section_value else {
                continue;
            };

            let default_section = defaults_table
                .get(section_name)
                .and_then(|v| v.as_table())
                .cloned()
                .unwrap_or_default();

            self.write_section(&mut output, section_name, section_table, &default_section);
        }

        if output.is_empty() && self.options.no_defaults {
            println!("# No non-default configuration values found.");
        } else {
            print!("{output}");
        }
    }
}

pub fn print_config_with_options(settings: &Settings, options: &ShowConfigOptions) {
    let printer = ConfigPrinter::new(settings, options);
    printer.print();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_value_string() {
        assert_eq!(format_value(&Value::String("hello".into())), "\"hello\"");
    }

    #[test]
    fn format_value_string_escapes_quotes() {
        assert_eq!(
            format_value(&Value::String("say \"hi\"".into())),
            "\"say \\\"hi\\\"\""
        );
    }

    #[test]
    fn format_value_string_escapes_backslashes() {
        assert_eq!(
            format_value(&Value::String("path\\to\\file".into())),
            "\"path\\\\to\\\\file\""
        );
    }

    #[test]
    fn format_value_integer() {
        assert_eq!(format_value(&Value::Integer(42)), "42");
    }

    #[test]
    fn format_value_boolean() {
        assert_eq!(format_value(&Value::Boolean(true)), "true");
        assert_eq!(format_value(&Value::Boolean(false)), "false");
    }

    #[test]
    fn format_value_array_empty() {
        assert_eq!(format_value(&Value::Array(vec![])), "[]");
    }

    #[test]
    fn format_value_array_strings() {
        let arr = Value::Array(vec![
            Value::String("one".into()),
            Value::String("two".into()),
        ]);
        assert_eq!(format_value(&arr), "[\"one\", \"two\"]");
    }

    #[test]
    fn source_comment_disabled() {
        let mut settings = Settings::default();
        settings
            .sources
            .insert("local.port".to_string(), ConfigSource::Toml);
        let options = ShowConfigOptions {
            no_defaults: false,
            show_sources: false,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        // When show_sources is false, no comment is shown
        assert_eq!(printer.source_comment("local.port"), "");
    }

    #[test]
    fn source_comment_enabled_default_is_empty() {
        let settings = Settings::default();
        let options = ShowConfigOptions {
            no_defaults: false,
            show_sources: true,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        // Default values should not show a source comment even when show_sources is enabled
        assert_eq!(printer.source_comment("local.port"), "");
    }

    #[test]
    fn source_comment_enabled_toml() {
        let mut settings = Settings::default();
        settings
            .sources
            .insert("local.port".to_string(), ConfigSource::Toml);
        let options = ShowConfigOptions {
            no_defaults: false,
            show_sources: true,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert_eq!(printer.source_comment("local.port"), " # source: toml");
    }

    #[test]
    fn source_comment_enabled_env() {
        let mut settings = Settings::default();
        settings
            .sources
            .insert("local.port".to_string(), ConfigSource::Env);
        let options = ShowConfigOptions {
            no_defaults: false,
            show_sources: true,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert_eq!(printer.source_comment("local.port"), " # source: env");
    }

    #[test]
    fn source_comment_enabled_cli() {
        let mut settings = Settings::default();
        settings
            .sources
            .insert("local.port".to_string(), ConfigSource::Cli);
        let options = ShowConfigOptions {
            no_defaults: false,
            show_sources: true,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert_eq!(printer.source_comment("local.port"), " # source: cli");
    }

    #[test]
    fn should_show_all_when_no_defaults_disabled() {
        let settings = Settings::default();
        let options = ShowConfigOptions {
            no_defaults: false,
            show_sources: false,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert!(printer.should_show("local.port"));
    }

    #[test]
    fn should_show_hides_defaults_when_enabled() {
        let settings = Settings::default();
        let options = ShowConfigOptions {
            no_defaults: true,
            show_sources: false,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert!(!printer.should_show("local.port"));
    }

    #[test]
    fn should_show_shows_toml_when_no_defaults() {
        let mut settings = Settings::default();
        settings
            .sources
            .insert("local.port".to_string(), ConfigSource::Toml);
        let options = ShowConfigOptions {
            no_defaults: true,
            show_sources: false,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert!(printer.should_show("local.port"));
    }

    #[test]
    fn should_show_shows_env_when_no_defaults() {
        let mut settings = Settings::default();
        settings
            .sources
            .insert("local.port".to_string(), ConfigSource::Env);
        let options = ShowConfigOptions {
            no_defaults: true,
            show_sources: false,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert!(printer.should_show("local.port"));
    }

    #[test]
    fn should_show_shows_cli_when_no_defaults() {
        let mut settings = Settings::default();
        settings
            .sources
            .insert("local.port".to_string(), ConfigSource::Cli);
        let options = ShowConfigOptions {
            no_defaults: true,
            show_sources: false,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert!(printer.should_show("local.port"));
    }
}
