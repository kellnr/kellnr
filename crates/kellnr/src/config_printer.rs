//! Config printing for `kellnr config show`.
//!
//! Walks the `SettingsProv` directly via [`provcfg::Provenance::walk_leaves`],
//! grouping leaves into sections by their first path segment. The walker hands
//! us `(dotted_path, &dyn erased_serde::Serialize, Category, is_secret)` per
//! leaf, no re-serialization of `Settings` to `toml::Value` required.

use std::collections::BTreeMap;
use std::fmt::Write;

use kellnr_settings::{Category, Provenance, SettingsProv, ShowConfigOptions};
use toml::Value;

/// Public entry point invoked from `main.rs`.
pub fn print_config_with_options(prov: &SettingsProv, options: &ShowConfigOptions) {
    let print = render(prov, options);
    if print.is_empty() {
        // `--no-defaults` and nothing was non-default.
        println!("# No non-default configuration values found.");
    } else {
        print!("{print}");
    }
}

/// Render the config to a TOML-shaped string. Pulled out as a pure function
/// so the unit tests can assert on the exact output without capturing stdout.
fn render(prov: &SettingsProv, options: &ShowConfigOptions) -> String {
    // Group leaves by section (the first path segment). `BTreeMap` for
    // deterministic ordering matching the previous toml::Value-based code.
    let mut sections: BTreeMap<String, BTreeMap<String, (Value, Category)>> = BTreeMap::new();

    prov.walk_leaves("", &mut |path, value, category, is_secret| {
        // Secrets (`#[configurable(secret)]` on the user struct) are never
        // emitted, `config show` is safe to log.
        if is_secret {
            return;
        }
        let Some((section, field)) = path.split_once('.') else {
            // Top-level leaf (none today, but possible if the schema grows).
            return;
        };
        // Convert the type-erased value to a toml::Value for formatting.
        let Ok(toml_value) = toml_from_erased(value) else {
            return;
        };
        sections
            .entry(section.to_string())
            .or_default()
            .insert(field.to_string(), (toml_value, category));
    });

    let mut out = String::new();
    for (section_name, fields) in &sections {
        let mut body = String::new();
        for (field_name, (value, category)) in fields {
            if !should_show(options, *category) {
                continue;
            }
            // Setup section: hide fields that still equal their default so we
            // don't print the literal admin password in plain text.
            if section_name == "setup" && *category == Category::Default {
                continue;
            }
            let comment = source_comment(options, *category);
            let _ = writeln!(body, "{field_name} = {}{comment}", format_value(value));
        }
        if !body.is_empty() {
            let _ = writeln!(out, "[{section_name}]");
            out.push_str(&body);
            out.push('\n');
        }
    }
    out
}

fn should_show(options: &ShowConfigOptions, category: Category) -> bool {
    if options.no_defaults {
        !matches!(category, Category::Default)
    } else {
        true
    }
}

fn source_comment(options: &ShowConfigOptions, category: Category) -> &'static str {
    if !options.show_sources {
        return "";
    }
    match category {
        // kellnr only loads from TOML files, so File maps to "toml".
        Category::File => " # source: toml",
        Category::Env => " # source: env",
        Category::Cli => " # source: cli",
        _ => "",
    }
}

/// Serialize an erased value through serde into a `toml::Value`. The toml crate
/// can directly consume any `serde::Serialize`, we shim through `erased_serde`
/// so the visitor signature stays type-erased.
fn toml_from_erased(
    value: &dyn kellnr_settings::erased_serde::Serialize,
) -> Result<Value, toml::ser::Error> {
    // Wrap into a Serialize newtype so `toml::Value::try_from` can pick it up.
    struct Erased<'a>(&'a dyn kellnr_settings::erased_serde::Serialize);
    impl serde::Serialize for Erased<'_> {
        fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
            kellnr_settings::erased_serde::serialize(self.0, ser)
        }
    }
    Value::try_from(Erased(value))
}

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
        Value::Table(_) => "{ ... }".to_string(), // Nested tables shouldn't appear at leaf level
        Value::Datetime(dt) => dt.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_value_string() {
        assert_eq!(format_value(&Value::String("hello".into())), "\"hello\"");
    }

    #[test]
    fn format_value_escapes_quotes_and_backslashes() {
        assert_eq!(
            format_value(&Value::String("say \"hi\"".into())),
            "\"say \\\"hi\\\"\""
        );
        assert_eq!(
            format_value(&Value::String("path\\to\\file".into())),
            "\"path\\\\to\\\\file\""
        );
    }

    #[test]
    fn format_value_primitives() {
        assert_eq!(format_value(&Value::Integer(42)), "42");
        assert_eq!(format_value(&Value::Boolean(true)), "true");
        assert_eq!(format_value(&Value::Boolean(false)), "false");
    }

    #[test]
    fn format_value_arrays() {
        assert_eq!(format_value(&Value::Array(vec![])), "[]");
        let arr = Value::Array(vec![
            Value::String("one".into()),
            Value::String("two".into()),
        ]);
        assert_eq!(format_value(&arr), "[\"one\", \"two\"]");
    }

    #[test]
    fn source_comment_off_when_disabled() {
        let opts = ShowConfigOptions {
            no_defaults: false,
            show_sources: false,
        };
        assert_eq!(source_comment(&opts, Category::Env), "");
    }

    #[test]
    fn source_comment_per_category() {
        let opts = ShowConfigOptions {
            no_defaults: false,
            show_sources: true,
        };
        assert_eq!(source_comment(&opts, Category::Default), "");
        assert_eq!(source_comment(&opts, Category::File), " # source: toml");
        assert_eq!(source_comment(&opts, Category::Env), " # source: env");
        assert_eq!(source_comment(&opts, Category::Cli), " # source: cli");
    }

    /// End-to-end check: build a `SettingsProv` via the real settings stack
    /// (with a tempfile-only TOML layer and no CLI overrides) and assert
    /// `render` produces the expected grouped TOML output with `# source:`
    /// annotations and that hidden-secret paths are redacted.
    #[test]
    fn render_emits_grouped_sections_with_source_comments() {
        use std::io::Write as _;

        let mut tomlfile = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tomlfile,
            "[local]\nport = 7777\n\n\
             [postgresql]\npwd = \"secret\"\n"
        )
        .unwrap();

        // The env layer reads `KELLNR_*` from the process env. We don't set
        // anything matching, so all non-defaulted values come from the TOML.
        let prov = kellnr_settings::build_prov_with_cli(Some(tomlfile.path()), None)
            .expect("build_prov_with_cli from tempfile");

        let opts = ShowConfigOptions {
            no_defaults: true,
            show_sources: true,
        };
        let output = render(&prov, &opts);

        assert!(
            output.contains("port = 7777 # source: toml"),
            "expected local.port to be tagged as toml-sourced, got:\n{output}"
        );
        assert!(
            output.contains("[local]"),
            "expected [local] header in output, got:\n{output}"
        );
        // Secret redaction: postgresql.pwd is on the ALWAYS_HIDDEN list.
        assert!(
            !output.contains("pwd ="),
            "postgresql.pwd must never appear in render output, got:\n{output}"
        );
        // no_defaults=true: sections that only carry default values are hidden.
        assert!(
            !output.contains("[docs]"),
            "[docs] should be hidden when --no-defaults is set, got:\n{output}"
        );
    }

    #[test]
    fn should_show_respects_no_defaults() {
        let opts_show_all = ShowConfigOptions {
            no_defaults: false,
            show_sources: false,
        };
        assert!(should_show(&opts_show_all, Category::Default));

        let opts_hide_defaults = ShowConfigOptions {
            no_defaults: true,
            show_sources: false,
        };
        assert!(!should_show(&opts_hide_defaults, Category::Default));
        assert!(should_show(&opts_hide_defaults, Category::File));
        assert!(should_show(&opts_hide_defaults, Category::Env));
        assert!(should_show(&opts_hide_defaults, Category::Cli));
    }
}
