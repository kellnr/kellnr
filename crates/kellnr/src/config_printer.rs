//! Config printing for `kellnr config show` command.

use std::fmt::Write;

use kellnr_settings::{ConfigSource, Settings, ShowConfigOptions};

fn fmt_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn fmt_opt_str(s: Option<&str>) -> String {
    match s {
        Some(v) => fmt_str(v),
        None => "# not set".to_string(),
    }
}

fn fmt_vec(v: &[String]) -> String {
    if v.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[{}]",
            v.iter().map(|s| fmt_str(s)).collect::<Vec<_>>().join(", ")
        )
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
            match self.settings.sources.get(key) {
                Some(ConfigSource::Default) => " # source: default",
                Some(ConfigSource::Toml) => " # source: toml",
                Some(ConfigSource::Env) => " # source: env",
                Some(ConfigSource::Cli) => " # source: cli",
                None => "",
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

    fn write_setup(&self, output: &mut String) {
        let defaults = Settings::default();
        let mut section = String::new();

        if self.should_show("setup.admin_pwd")
            && self.settings.setup.admin_pwd != defaults.setup.admin_pwd
        {
            let _ = writeln!(
                section,
                "admin_pwd = {}{}",
                fmt_str(&self.settings.setup.admin_pwd),
                self.source_comment("setup.admin_pwd")
            );
        }
        if self.should_show("setup.admin_token")
            && self.settings.setup.admin_token != defaults.setup.admin_token
        {
            let _ = writeln!(
                section,
                "admin_token = {}{}",
                fmt_opt_str(self.settings.setup.admin_token.as_deref()),
                self.source_comment("setup.admin_token")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[setup]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_registry(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.registry;

        if self.should_show("registry.data_dir") {
            let _ = writeln!(
                section,
                "data_dir = {}{}",
                fmt_str(&s.data_dir),
                self.source_comment("registry.data_dir")
            );
        }
        if self.should_show("registry.session_age_seconds") {
            let _ = writeln!(
                section,
                "session_age_seconds = {}{}",
                s.session_age_seconds,
                self.source_comment("registry.session_age_seconds")
            );
        }
        if self.should_show("registry.cache_size") {
            let _ = writeln!(
                section,
                "cache_size = {}{}",
                s.cache_size,
                self.source_comment("registry.cache_size")
            );
        }
        if self.should_show("registry.max_crate_size") {
            let _ = writeln!(
                section,
                "max_crate_size = {}{}",
                s.max_crate_size,
                self.source_comment("registry.max_crate_size")
            );
        }
        if self.should_show("registry.max_db_connections") {
            let _ = writeln!(
                section,
                "max_db_connections = {}{}",
                s.max_db_connections,
                self.source_comment("registry.max_db_connections")
            );
        }
        if self.should_show("registry.auth_required") {
            let _ = writeln!(
                section,
                "auth_required = {}{}",
                s.auth_required,
                self.source_comment("registry.auth_required")
            );
        }
        if self.should_show("registry.required_crate_fields") {
            let _ = writeln!(
                section,
                "required_crate_fields = {}{}",
                fmt_vec(&s.required_crate_fields),
                self.source_comment("registry.required_crate_fields")
            );
        }
        if self.should_show("registry.new_crates_restricted") {
            let _ = writeln!(
                section,
                "new_crates_restricted = {}{}",
                s.new_crates_restricted,
                self.source_comment("registry.new_crates_restricted")
            );
        }
        if self.should_show("registry.cookie_signing_key") {
            let _ = writeln!(
                section,
                "cookie_signing_key = {}{}",
                fmt_opt_str(s.cookie_signing_key.as_deref()),
                self.source_comment("registry.cookie_signing_key")
            );
        }
        if self.should_show("registry.allow_ownerless_crates") {
            let _ = writeln!(
                section,
                "allow_ownerless_crates = {}{}",
                s.allow_ownerless_crates,
                self.source_comment("registry.allow_ownerless_crates")
            );
        }
        if self.should_show("registry.token_cache_enabled") {
            let _ = writeln!(
                section,
                "token_cache_enabled = {}{}",
                s.token_cache_enabled,
                self.source_comment("registry.token_cache_enabled")
            );
        }
        if self.should_show("registry.token_cache_ttl_seconds") {
            let _ = writeln!(
                section,
                "token_cache_ttl_seconds = {}{}",
                s.token_cache_ttl_seconds,
                self.source_comment("registry.token_cache_ttl_seconds")
            );
        }
        if self.should_show("registry.token_cache_max_capacity") {
            let _ = writeln!(
                section,
                "token_cache_max_capacity = {}{}",
                s.token_cache_max_capacity,
                self.source_comment("registry.token_cache_max_capacity")
            );
        }
        if self.should_show("registry.token_db_retry_count") {
            let _ = writeln!(
                section,
                "token_db_retry_count = {}{}",
                s.token_db_retry_count,
                self.source_comment("registry.token_db_retry_count")
            );
        }
        if self.should_show("registry.token_db_retry_delay_ms") {
            let _ = writeln!(
                section,
                "token_db_retry_delay_ms = {}{}",
                s.token_db_retry_delay_ms,
                self.source_comment("registry.token_db_retry_delay_ms")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[registry]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_docs(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.docs;

        if self.should_show("docs.enabled") {
            let _ = writeln!(
                section,
                "enabled = {}{}",
                s.enabled,
                self.source_comment("docs.enabled")
            );
        }
        if self.should_show("docs.max_size") {
            let _ = writeln!(
                section,
                "max_size = {}{}",
                s.max_size,
                self.source_comment("docs.max_size")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[docs]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_proxy(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.proxy;

        if self.should_show("proxy.enabled") {
            let _ = writeln!(
                section,
                "enabled = {}{}",
                s.enabled,
                self.source_comment("proxy.enabled")
            );
        }
        if self.should_show("proxy.num_threads") {
            let _ = writeln!(
                section,
                "num_threads = {}{}",
                s.num_threads,
                self.source_comment("proxy.num_threads")
            );
        }
        if self.should_show("proxy.download_on_update") {
            let _ = writeln!(
                section,
                "download_on_update = {}{}",
                s.download_on_update,
                self.source_comment("proxy.download_on_update")
            );
        }
        if self.should_show("proxy.url") {
            let _ = writeln!(
                section,
                "url = {}{}",
                fmt_str(s.url.as_str()),
                self.source_comment("proxy.url")
            );
        }
        if self.should_show("proxy.index") {
            let _ = writeln!(
                section,
                "index = {}{}",
                fmt_str(s.index.as_str()),
                self.source_comment("proxy.index")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[proxy]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_log(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.log;

        if self.should_show("log.level") {
            let _ = writeln!(
                section,
                "level = {}{}",
                fmt_str(&format!("{:?}", s.level).to_lowercase()),
                self.source_comment("log.level")
            );
        }
        if self.should_show("log.format") {
            let _ = writeln!(
                section,
                "format = {}{}",
                fmt_str(&format!("{:?}", s.format).to_lowercase()),
                self.source_comment("log.format")
            );
        }
        if self.should_show("log.level_web_server") {
            let _ = writeln!(
                section,
                "level_web_server = {}{}",
                fmt_str(&format!("{:?}", s.level_web_server).to_lowercase()),
                self.source_comment("log.level_web_server")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[log]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_local(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.local;

        if self.should_show("local.ip") {
            let _ = writeln!(
                section,
                "ip = {}{}",
                fmt_str(&s.ip.to_string()),
                self.source_comment("local.ip")
            );
        }
        if self.should_show("local.port") {
            let _ = writeln!(
                section,
                "port = {}{}",
                s.port,
                self.source_comment("local.port")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[local]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_origin(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.origin;

        if self.should_show("origin.hostname") {
            let _ = writeln!(
                section,
                "hostname = {}{}",
                fmt_str(&s.hostname),
                self.source_comment("origin.hostname")
            );
        }
        if self.should_show("origin.port") {
            let _ = writeln!(
                section,
                "port = {}{}",
                s.port,
                self.source_comment("origin.port")
            );
        }
        if self.should_show("origin.protocol") {
            let _ = writeln!(
                section,
                "protocol = {}{}",
                fmt_str(&format!("{:?}", s.protocol).to_lowercase()),
                self.source_comment("origin.protocol")
            );
        }
        if self.should_show("origin.path") {
            let _ = writeln!(
                section,
                "path = {}{}",
                fmt_str(&s.path),
                self.source_comment("origin.path")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[origin]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_postgresql(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.postgresql;

        if self.should_show("postgresql.enabled") {
            let _ = writeln!(
                section,
                "enabled = {}{}",
                s.enabled,
                self.source_comment("postgresql.enabled")
            );
        }
        if self.should_show("postgresql.address") {
            let _ = writeln!(
                section,
                "address = {}{}",
                fmt_str(&s.address),
                self.source_comment("postgresql.address")
            );
        }
        if self.should_show("postgresql.port") {
            let _ = writeln!(
                section,
                "port = {}{}",
                s.port,
                self.source_comment("postgresql.port")
            );
        }
        if self.should_show("postgresql.db") {
            let _ = writeln!(
                section,
                "db = {}{}",
                fmt_str(&s.db),
                self.source_comment("postgresql.db")
            );
        }
        if self.should_show("postgresql.user") {
            let _ = writeln!(
                section,
                "user = {}{}",
                fmt_str(&s.user),
                self.source_comment("postgresql.user")
            );
        }
        if self.should_show("postgresql.pwd") {
            let _ = writeln!(
                section,
                "pwd = {}{}",
                fmt_str(&s.pwd),
                self.source_comment("postgresql.pwd")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[postgresql]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_s3(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.s3;

        if self.should_show("s3.enabled") {
            let _ = writeln!(
                section,
                "enabled = {}{}",
                s.enabled,
                self.source_comment("s3.enabled")
            );
        }
        if self.should_show("s3.access_key") {
            let _ = writeln!(
                section,
                "access_key = {}{}",
                fmt_str(&s.access_key),
                self.source_comment("s3.access_key")
            );
        }
        if self.should_show("s3.secret_key") {
            let _ = writeln!(
                section,
                "secret_key = {}{}",
                fmt_str(&s.secret_key),
                self.source_comment("s3.secret_key")
            );
        }
        if self.should_show("s3.region") {
            let _ = writeln!(
                section,
                "region = {}{}",
                fmt_str(&s.region),
                self.source_comment("s3.region")
            );
        }
        if self.should_show("s3.endpoint") {
            let _ = writeln!(
                section,
                "endpoint = {}{}",
                fmt_str(&s.endpoint),
                self.source_comment("s3.endpoint")
            );
        }
        if self.should_show("s3.allow_http") {
            let _ = writeln!(
                section,
                "allow_http = {}{}",
                s.allow_http,
                self.source_comment("s3.allow_http")
            );
        }
        if self.should_show("s3.crates_bucket") {
            let _ = writeln!(
                section,
                "crates_bucket = {}{}",
                fmt_str(&s.crates_bucket),
                self.source_comment("s3.crates_bucket")
            );
        }
        if self.should_show("s3.cratesio_bucket") {
            let _ = writeln!(
                section,
                "cratesio_bucket = {}{}",
                fmt_str(&s.cratesio_bucket),
                self.source_comment("s3.cratesio_bucket")
            );
        }
        if self.should_show("s3.toolchain_bucket") {
            let _ = writeln!(
                section,
                "toolchain_bucket = {}{}",
                fmt_str(&s.toolchain_bucket),
                self.source_comment("s3.toolchain_bucket")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[s3]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_oauth2(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.oauth2;

        if self.should_show("oauth2.enabled") {
            let _ = writeln!(
                section,
                "enabled = {}{}",
                s.enabled,
                self.source_comment("oauth2.enabled")
            );
        }
        if self.should_show("oauth2.issuer_url") {
            let _ = writeln!(
                section,
                "issuer_url = {}{}",
                fmt_opt_str(s.issuer_url.as_deref()),
                self.source_comment("oauth2.issuer_url")
            );
        }
        if self.should_show("oauth2.client_id") {
            let _ = writeln!(
                section,
                "client_id = {}{}",
                fmt_opt_str(s.client_id.as_deref()),
                self.source_comment("oauth2.client_id")
            );
        }
        if self.should_show("oauth2.client_secret") {
            let _ = writeln!(
                section,
                "client_secret = {}{}",
                fmt_opt_str(s.client_secret.as_deref()),
                self.source_comment("oauth2.client_secret")
            );
        }
        if self.should_show("oauth2.scopes") {
            let _ = writeln!(
                section,
                "scopes = {}{}",
                fmt_vec(&s.scopes),
                self.source_comment("oauth2.scopes")
            );
        }
        if self.should_show("oauth2.auto_provision_users") {
            let _ = writeln!(
                section,
                "auto_provision_users = {}{}",
                s.auto_provision_users,
                self.source_comment("oauth2.auto_provision_users")
            );
        }
        if self.should_show("oauth2.admin_group_claim") {
            let _ = writeln!(
                section,
                "admin_group_claim = {}{}",
                fmt_opt_str(s.admin_group_claim.as_deref()),
                self.source_comment("oauth2.admin_group_claim")
            );
        }
        if self.should_show("oauth2.admin_group_value") {
            let _ = writeln!(
                section,
                "admin_group_value = {}{}",
                fmt_opt_str(s.admin_group_value.as_deref()),
                self.source_comment("oauth2.admin_group_value")
            );
        }
        if self.should_show("oauth2.read_only_group_claim") {
            let _ = writeln!(
                section,
                "read_only_group_claim = {}{}",
                fmt_opt_str(s.read_only_group_claim.as_deref()),
                self.source_comment("oauth2.read_only_group_claim")
            );
        }
        if self.should_show("oauth2.read_only_group_value") {
            let _ = writeln!(
                section,
                "read_only_group_value = {}{}",
                fmt_opt_str(s.read_only_group_value.as_deref()),
                self.source_comment("oauth2.read_only_group_value")
            );
        }
        if self.should_show("oauth2.button_text") {
            let _ = writeln!(
                section,
                "button_text = {}{}",
                fmt_str(&s.button_text),
                self.source_comment("oauth2.button_text")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[oauth2]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn write_toolchain(&self, output: &mut String) {
        let mut section = String::new();
        let s = &self.settings.toolchain;

        if self.should_show("toolchain.enabled") {
            let _ = writeln!(
                section,
                "enabled = {}{}",
                s.enabled,
                self.source_comment("toolchain.enabled")
            );
        }
        if self.should_show("toolchain.max_size") {
            let _ = writeln!(
                section,
                "max_size = {}{}",
                s.max_size,
                self.source_comment("toolchain.max_size")
            );
        }

        if !section.is_empty() {
            let _ = writeln!(output, "[toolchain]");
            output.push_str(&section);
            output.push('\n');
        }
    }

    fn print(&self) {
        let mut output = String::new();

        self.write_setup(&mut output);
        self.write_registry(&mut output);
        self.write_docs(&mut output);
        self.write_proxy(&mut output);
        self.write_log(&mut output);
        self.write_local(&mut output);
        self.write_origin(&mut output);
        self.write_postgresql(&mut output);
        self.write_s3(&mut output);
        self.write_oauth2(&mut output);
        self.write_toolchain(&mut output);

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
    fn fmt_str_escapes_quotes() {
        assert_eq!(fmt_str("hello"), "\"hello\"");
        assert_eq!(fmt_str("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn fmt_str_escapes_backslashes() {
        assert_eq!(fmt_str("path\\to\\file"), "\"path\\\\to\\\\file\"");
    }

    #[test]
    fn fmt_opt_str_handles_none() {
        assert_eq!(fmt_opt_str(None), "# not set");
    }

    #[test]
    fn fmt_opt_str_handles_some() {
        assert_eq!(fmt_opt_str(Some("value")), "\"value\"");
    }

    #[test]
    fn fmt_vec_empty() {
        let v: Vec<String> = vec![];
        assert_eq!(fmt_vec(&v), "[]");
    }

    #[test]
    fn fmt_vec_single() {
        let v = vec!["one".to_string()];
        assert_eq!(fmt_vec(&v), "[\"one\"]");
    }

    #[test]
    fn fmt_vec_multiple() {
        let v = vec!["one".to_string(), "two".to_string()];
        assert_eq!(fmt_vec(&v), "[\"one\", \"two\"]");
    }

    #[test]
    fn source_comment_disabled() {
        let settings = Settings::default();
        let options = ShowConfigOptions {
            no_defaults: false,
            show_sources: false,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert_eq!(printer.source_comment("local.port"), "");
    }

    #[test]
    fn source_comment_enabled_default() {
        let settings = Settings::default();
        let options = ShowConfigOptions {
            no_defaults: false,
            show_sources: true,
        };
        let printer = ConfigPrinter::new(&settings, &options);
        assert_eq!(printer.source_comment("local.port"), " # source: default");
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
