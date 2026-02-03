use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use clap_serde_derive::ClapSerde;
use config::ConfigError;

use crate::compile_time_config;
use crate::config_source::{ConfigSource, track_env_sources};
use crate::docs::Docs;
use crate::local::Local;
use crate::log::{LogFormat, LogLevel};
use crate::oauth2::OAuth2;
use crate::origin::Origin;
use crate::postgresql::Postgresql;
use crate::proxy::Proxy;
use crate::registry::Registry;
use crate::s3::S3;
use crate::settings::Settings;
use crate::setup::Setup;
use crate::toolchain::Toolchain;

#[derive(Parser)]
#[command(name = "kellnr", version, about)]
pub struct Cli {
    /// Path to configuration file
    #[arg(id = "config", short = 'c', long = "config", global = true)]
    pub config_file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Server configuration options
#[derive(Parser, Default)]
pub struct ServerArgs {
    #[command(flatten)]
    pub local: <Local as ClapSerde>::Opt,

    #[command(flatten)]
    pub origin: <Origin as ClapSerde>::Opt,

    #[command(flatten)]
    pub registry: <Registry as ClapSerde>::Opt,

    #[command(flatten)]
    pub docs: <Docs as ClapSerde>::Opt,

    #[command(flatten)]
    pub proxy: <Proxy as ClapSerde>::Opt,

    #[command(flatten)]
    pub postgresql: <Postgresql as ClapSerde>::Opt,

    #[command(flatten)]
    pub s3: <S3 as ClapSerde>::Opt,

    #[command(flatten)]
    pub setup: <Setup as ClapSerde>::Opt,

    #[command(flatten)]
    pub oauth2: <OAuth2 as ClapSerde>::Opt,

    #[command(flatten)]
    pub toolchain: <Toolchain as ClapSerde>::Opt,

    // Log settings (manual, not using ClapSerde due to custom serde deserializers)
    /// Log output format
    #[arg(id = "log-format", long = "log-format", value_enum)]
    pub log_format: Option<LogFormat>,

    /// Log level
    #[arg(id = "log-level", short = 'l', long = "log-level", value_enum)]
    pub log_level: Option<LogLevel>,

    /// Log level for web server
    #[arg(id = "log-level-web-server", long = "log-level-web-server", value_enum)]
    pub log_level_web_server: Option<LogLevel>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    /// Start the kellnr server
    Start {
        #[command(flatten)]
        server: ServerArgs,
    },
    /// Configuration management commands
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show the current configuration
    Show {
        /// Hide settings that have their default value
        #[arg(long = "no-defaults")]
        no_defaults: bool,

        /// Show the source (toml, env, cli) for each setting
        #[arg(long = "sources")]
        show_sources: bool,
    },
    /// Initialize a new configuration file with default values
    Init {
        /// Output file path (default: ./kellnr.toml)
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,
    },
}

/// Options for the `config show` command
#[derive(Debug, Clone, Default)]
pub struct ShowConfigOptions {
    /// Hide settings that have their default value
    pub no_defaults: bool,
    /// Show the source (toml, env, cli) for each setting
    pub show_sources: bool,
}

pub enum CliResult {
    RunServer(Settings),
    ShowConfig {
        settings: Settings,
        options: ShowConfigOptions,
    },
    InitConfig {
        settings: Settings,
        output: PathBuf,
    },
    ShowHelp,
}

pub fn parse_cli() -> Result<CliResult, ConfigError> {
    let cli = Cli::parse();

    // Handle `config init` early - it doesn't need an existing config file
    if let Some(Command::Config {
        action: ConfigAction::Init { output },
    }) = &cli.command
    {
        let output_path = output
            .clone()
            .unwrap_or_else(|| PathBuf::from("kellnr.toml"));
        return Ok(CliResult::InitConfig {
            settings: Settings::default(),
            output: output_path,
        });
    }

    // Config file priority: CLI > env var > compile-time > None
    let env_config_file = std::env::var("KELLNR_CONFIG_FILE").ok();
    let config_file: Option<PathBuf> = cli
        .config_file
        .or(env_config_file.map(PathBuf::from))
        .or(compile_time_config::KELLNR_COMPTIME__CONFIG_FILE.map(PathBuf::from));

    // Load settings from file + env (sources are initialized to Default)
    let mut settings = Settings::try_from(config_file.as_deref())?;

    // Track TOML sources (compare to defaults)
    if config_file.is_some() {
        track_toml_sources(&mut settings);
    }

    // Track ENV sources (check which env vars are set)
    // This must come after TOML tracking since ENV overrides TOML
    track_env_sources(&mut settings.sources);

    // Handle subcommands
    match cli.command {
        Some(Command::Start { server }) => {
            // Track and merge CLI args
            track_and_merge_cli(&mut settings, server);

            Ok(CliResult::RunServer(settings))
        }
        Some(Command::Config { action }) => match action {
            ConfigAction::Show {
                no_defaults,
                show_sources,
            } => Ok(CliResult::ShowConfig {
                settings,
                options: ShowConfigOptions {
                    no_defaults,
                    show_sources,
                },
            }),
            ConfigAction::Init { .. } => {
                // Already handled above
                unreachable!()
            }
        },
        None => {
            // No subcommand - show help
            Cli::command().print_help().ok();
            Ok(CliResult::ShowHelp)
        }
    }
}

/// Legacy function for backward compatibility
pub fn get_settings_with_cli() -> Result<Settings, ConfigError> {
    match parse_cli()? {
        CliResult::RunServer(settings)
        | CliResult::ShowConfig { settings, .. }
        | CliResult::InitConfig { settings, .. } => Ok(settings),
        CliResult::ShowHelp => Ok(Settings::default()),
    }
}

/// Compare current settings to defaults and mark non-default values as TOML-sourced.
fn track_toml_sources(settings: &mut Settings) {
    let defaults = Settings::default();

    // Registry settings
    if settings.registry.data_dir != defaults.registry.data_dir {
        settings
            .sources
            .insert("registry.data_dir".to_string(), ConfigSource::Toml);
    }
    if settings.registry.session_age_seconds != defaults.registry.session_age_seconds {
        settings.sources.insert(
            "registry.session_age_seconds".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.cache_size != defaults.registry.cache_size {
        settings
            .sources
            .insert("registry.cache_size".to_string(), ConfigSource::Toml);
    }
    if settings.registry.max_crate_size != defaults.registry.max_crate_size {
        settings
            .sources
            .insert("registry.max_crate_size".to_string(), ConfigSource::Toml);
    }
    if settings.registry.max_db_connections != defaults.registry.max_db_connections {
        settings.sources.insert(
            "registry.max_db_connections".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.auth_required != defaults.registry.auth_required {
        settings
            .sources
            .insert("registry.auth_required".to_string(), ConfigSource::Toml);
    }
    if settings.registry.required_crate_fields != defaults.registry.required_crate_fields {
        settings.sources.insert(
            "registry.required_crate_fields".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.new_crates_restricted != defaults.registry.new_crates_restricted {
        settings.sources.insert(
            "registry.new_crates_restricted".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.cookie_signing_key != defaults.registry.cookie_signing_key {
        settings.sources.insert(
            "registry.cookie_signing_key".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.allow_ownerless_crates != defaults.registry.allow_ownerless_crates {
        settings.sources.insert(
            "registry.allow_ownerless_crates".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.token_cache_enabled != defaults.registry.token_cache_enabled {
        settings.sources.insert(
            "registry.token_cache_enabled".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.token_cache_ttl_seconds != defaults.registry.token_cache_ttl_seconds {
        settings.sources.insert(
            "registry.token_cache_ttl_seconds".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.token_cache_max_capacity != defaults.registry.token_cache_max_capacity {
        settings.sources.insert(
            "registry.token_cache_max_capacity".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.token_db_retry_count != defaults.registry.token_db_retry_count {
        settings.sources.insert(
            "registry.token_db_retry_count".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.registry.token_db_retry_delay_ms != defaults.registry.token_db_retry_delay_ms {
        settings.sources.insert(
            "registry.token_db_retry_delay_ms".to_string(),
            ConfigSource::Toml,
        );
    }

    // Local settings
    if settings.local.ip != defaults.local.ip {
        settings
            .sources
            .insert("local.ip".to_string(), ConfigSource::Toml);
    }
    if settings.local.port != defaults.local.port {
        settings
            .sources
            .insert("local.port".to_string(), ConfigSource::Toml);
    }

    // Origin settings
    if settings.origin.hostname != defaults.origin.hostname {
        settings
            .sources
            .insert("origin.hostname".to_string(), ConfigSource::Toml);
    }
    if settings.origin.port != defaults.origin.port {
        settings
            .sources
            .insert("origin.port".to_string(), ConfigSource::Toml);
    }
    if settings.origin.protocol != defaults.origin.protocol {
        settings
            .sources
            .insert("origin.protocol".to_string(), ConfigSource::Toml);
    }
    if settings.origin.path != defaults.origin.path {
        settings
            .sources
            .insert("origin.path".to_string(), ConfigSource::Toml);
    }

    // Log settings
    if settings.log.level != defaults.log.level {
        settings
            .sources
            .insert("log.level".to_string(), ConfigSource::Toml);
    }
    if settings.log.format != defaults.log.format {
        settings
            .sources
            .insert("log.format".to_string(), ConfigSource::Toml);
    }
    if settings.log.level_web_server != defaults.log.level_web_server {
        settings
            .sources
            .insert("log.level_web_server".to_string(), ConfigSource::Toml);
    }

    // Docs settings
    if settings.docs.enabled != defaults.docs.enabled {
        settings
            .sources
            .insert("docs.enabled".to_string(), ConfigSource::Toml);
    }
    if settings.docs.max_size != defaults.docs.max_size {
        settings
            .sources
            .insert("docs.max_size".to_string(), ConfigSource::Toml);
    }

    // Proxy settings
    if settings.proxy.enabled != defaults.proxy.enabled {
        settings
            .sources
            .insert("proxy.enabled".to_string(), ConfigSource::Toml);
    }
    if settings.proxy.num_threads != defaults.proxy.num_threads {
        settings
            .sources
            .insert("proxy.num_threads".to_string(), ConfigSource::Toml);
    }
    if settings.proxy.download_on_update != defaults.proxy.download_on_update {
        settings
            .sources
            .insert("proxy.download_on_update".to_string(), ConfigSource::Toml);
    }
    if settings.proxy.url != defaults.proxy.url {
        settings
            .sources
            .insert("proxy.url".to_string(), ConfigSource::Toml);
    }
    if settings.proxy.index != defaults.proxy.index {
        settings
            .sources
            .insert("proxy.index".to_string(), ConfigSource::Toml);
    }

    // PostgreSQL settings
    if settings.postgresql.enabled != defaults.postgresql.enabled {
        settings
            .sources
            .insert("postgresql.enabled".to_string(), ConfigSource::Toml);
    }
    if settings.postgresql.address != defaults.postgresql.address {
        settings
            .sources
            .insert("postgresql.address".to_string(), ConfigSource::Toml);
    }
    if settings.postgresql.port != defaults.postgresql.port {
        settings
            .sources
            .insert("postgresql.port".to_string(), ConfigSource::Toml);
    }
    if settings.postgresql.db != defaults.postgresql.db {
        settings
            .sources
            .insert("postgresql.db".to_string(), ConfigSource::Toml);
    }
    if settings.postgresql.user != defaults.postgresql.user {
        settings
            .sources
            .insert("postgresql.user".to_string(), ConfigSource::Toml);
    }
    if settings.postgresql.pwd != defaults.postgresql.pwd {
        settings
            .sources
            .insert("postgresql.pwd".to_string(), ConfigSource::Toml);
    }

    // S3 settings
    if settings.s3.enabled != defaults.s3.enabled {
        settings
            .sources
            .insert("s3.enabled".to_string(), ConfigSource::Toml);
    }
    if settings.s3.access_key != defaults.s3.access_key {
        settings
            .sources
            .insert("s3.access_key".to_string(), ConfigSource::Toml);
    }
    if settings.s3.secret_key != defaults.s3.secret_key {
        settings
            .sources
            .insert("s3.secret_key".to_string(), ConfigSource::Toml);
    }
    if settings.s3.region != defaults.s3.region {
        settings
            .sources
            .insert("s3.region".to_string(), ConfigSource::Toml);
    }
    if settings.s3.endpoint != defaults.s3.endpoint {
        settings
            .sources
            .insert("s3.endpoint".to_string(), ConfigSource::Toml);
    }
    if settings.s3.allow_http != defaults.s3.allow_http {
        settings
            .sources
            .insert("s3.allow_http".to_string(), ConfigSource::Toml);
    }
    if settings.s3.crates_bucket != defaults.s3.crates_bucket {
        settings
            .sources
            .insert("s3.crates_bucket".to_string(), ConfigSource::Toml);
    }
    if settings.s3.cratesio_bucket != defaults.s3.cratesio_bucket {
        settings
            .sources
            .insert("s3.cratesio_bucket".to_string(), ConfigSource::Toml);
    }
    if settings.s3.toolchain_bucket != defaults.s3.toolchain_bucket {
        settings
            .sources
            .insert("s3.toolchain_bucket".to_string(), ConfigSource::Toml);
    }

    // Setup settings
    if settings.setup.admin_pwd != defaults.setup.admin_pwd {
        settings
            .sources
            .insert("setup.admin_pwd".to_string(), ConfigSource::Toml);
    }
    if settings.setup.admin_token != defaults.setup.admin_token {
        settings
            .sources
            .insert("setup.admin_token".to_string(), ConfigSource::Toml);
    }

    // OAuth2 settings
    if settings.oauth2.enabled != defaults.oauth2.enabled {
        settings
            .sources
            .insert("oauth2.enabled".to_string(), ConfigSource::Toml);
    }
    if settings.oauth2.issuer_url != defaults.oauth2.issuer_url {
        settings
            .sources
            .insert("oauth2.issuer_url".to_string(), ConfigSource::Toml);
    }
    if settings.oauth2.client_id != defaults.oauth2.client_id {
        settings
            .sources
            .insert("oauth2.client_id".to_string(), ConfigSource::Toml);
    }
    if settings.oauth2.client_secret != defaults.oauth2.client_secret {
        settings
            .sources
            .insert("oauth2.client_secret".to_string(), ConfigSource::Toml);
    }
    if settings.oauth2.scopes != defaults.oauth2.scopes {
        settings
            .sources
            .insert("oauth2.scopes".to_string(), ConfigSource::Toml);
    }
    if settings.oauth2.auto_provision_users != defaults.oauth2.auto_provision_users {
        settings.sources.insert(
            "oauth2.auto_provision_users".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.oauth2.admin_group_claim != defaults.oauth2.admin_group_claim {
        settings
            .sources
            .insert("oauth2.admin_group_claim".to_string(), ConfigSource::Toml);
    }
    if settings.oauth2.admin_group_value != defaults.oauth2.admin_group_value {
        settings
            .sources
            .insert("oauth2.admin_group_value".to_string(), ConfigSource::Toml);
    }
    if settings.oauth2.read_only_group_claim != defaults.oauth2.read_only_group_claim {
        settings.sources.insert(
            "oauth2.read_only_group_claim".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.oauth2.read_only_group_value != defaults.oauth2.read_only_group_value {
        settings.sources.insert(
            "oauth2.read_only_group_value".to_string(),
            ConfigSource::Toml,
        );
    }
    if settings.oauth2.button_text != defaults.oauth2.button_text {
        settings
            .sources
            .insert("oauth2.button_text".to_string(), ConfigSource::Toml);
    }

    // Toolchain settings
    if settings.toolchain.enabled != defaults.toolchain.enabled {
        settings
            .sources
            .insert("toolchain.enabled".to_string(), ConfigSource::Toml);
    }
    if settings.toolchain.max_size != defaults.toolchain.max_size {
        settings
            .sources
            .insert("toolchain.max_size".to_string(), ConfigSource::Toml);
    }
}

/// Track CLI arguments and merge them into settings.
fn track_and_merge_cli(settings: &mut Settings, server: ServerArgs) {
    // Local settings
    if server.local.ip.is_some() {
        settings
            .sources
            .insert("local.ip".to_string(), ConfigSource::Cli);
    }
    if server.local.port.is_some() {
        settings
            .sources
            .insert("local.port".to_string(), ConfigSource::Cli);
    }
    settings.local = settings.local.clone().merge(server.local);

    // Origin settings
    if server.origin.hostname.is_some() {
        settings
            .sources
            .insert("origin.hostname".to_string(), ConfigSource::Cli);
    }
    if server.origin.port.is_some() {
        settings
            .sources
            .insert("origin.port".to_string(), ConfigSource::Cli);
    }
    if server.origin.path.is_some() {
        settings
            .sources
            .insert("origin.path".to_string(), ConfigSource::Cli);
    }
    settings.origin = settings.origin.clone().merge(server.origin);

    // Registry settings
    if server.registry.data_dir.is_some() {
        settings
            .sources
            .insert("registry.data_dir".to_string(), ConfigSource::Cli);
    }
    if server.registry.session_age_seconds.is_some() {
        settings.sources.insert(
            "registry.session_age_seconds".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.registry.cache_size.is_some() {
        settings
            .sources
            .insert("registry.cache_size".to_string(), ConfigSource::Cli);
    }
    if server.registry.max_crate_size.is_some() {
        settings
            .sources
            .insert("registry.max_crate_size".to_string(), ConfigSource::Cli);
    }
    if server.registry.max_db_connections.is_some() {
        settings
            .sources
            .insert("registry.max_db_connections".to_string(), ConfigSource::Cli);
    }
    if server.registry.auth_required.is_some() {
        settings
            .sources
            .insert("registry.auth_required".to_string(), ConfigSource::Cli);
    }
    if server.registry.required_crate_fields.is_some() {
        settings.sources.insert(
            "registry.required_crate_fields".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.registry.new_crates_restricted.is_some() {
        settings.sources.insert(
            "registry.new_crates_restricted".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.registry.cookie_signing_key.is_some() {
        settings
            .sources
            .insert("registry.cookie_signing_key".to_string(), ConfigSource::Cli);
    }
    if server.registry.allow_ownerless_crates.is_some() {
        settings.sources.insert(
            "registry.allow_ownerless_crates".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.registry.token_cache_enabled.is_some() {
        settings.sources.insert(
            "registry.token_cache_enabled".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.registry.token_cache_ttl_seconds.is_some() {
        settings.sources.insert(
            "registry.token_cache_ttl_seconds".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.registry.token_cache_max_capacity.is_some() {
        settings.sources.insert(
            "registry.token_cache_max_capacity".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.registry.token_db_retry_count.is_some() {
        settings.sources.insert(
            "registry.token_db_retry_count".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.registry.token_db_retry_delay_ms.is_some() {
        settings.sources.insert(
            "registry.token_db_retry_delay_ms".to_string(),
            ConfigSource::Cli,
        );
    }
    settings.registry = settings.registry.clone().merge(server.registry);

    // Docs settings
    if server.docs.enabled.is_some() {
        settings
            .sources
            .insert("docs.enabled".to_string(), ConfigSource::Cli);
    }
    if server.docs.max_size.is_some() {
        settings
            .sources
            .insert("docs.max_size".to_string(), ConfigSource::Cli);
    }
    settings.docs = settings.docs.clone().merge(server.docs);

    // Proxy settings
    if server.proxy.enabled.is_some() {
        settings
            .sources
            .insert("proxy.enabled".to_string(), ConfigSource::Cli);
    }
    if server.proxy.num_threads.is_some() {
        settings
            .sources
            .insert("proxy.num_threads".to_string(), ConfigSource::Cli);
    }
    if server.proxy.download_on_update.is_some() {
        settings
            .sources
            .insert("proxy.download_on_update".to_string(), ConfigSource::Cli);
    }
    if server.proxy.url.is_some() {
        settings
            .sources
            .insert("proxy.url".to_string(), ConfigSource::Cli);
    }
    if server.proxy.index.is_some() {
        settings
            .sources
            .insert("proxy.index".to_string(), ConfigSource::Cli);
    }
    settings.proxy = settings.proxy.clone().merge(server.proxy);

    // PostgreSQL settings
    if server.postgresql.enabled.is_some() {
        settings
            .sources
            .insert("postgresql.enabled".to_string(), ConfigSource::Cli);
    }
    if server.postgresql.address.is_some() {
        settings
            .sources
            .insert("postgresql.address".to_string(), ConfigSource::Cli);
    }
    if server.postgresql.port.is_some() {
        settings
            .sources
            .insert("postgresql.port".to_string(), ConfigSource::Cli);
    }
    if server.postgresql.db.is_some() {
        settings
            .sources
            .insert("postgresql.db".to_string(), ConfigSource::Cli);
    }
    if server.postgresql.user.is_some() {
        settings
            .sources
            .insert("postgresql.user".to_string(), ConfigSource::Cli);
    }
    if server.postgresql.pwd.is_some() {
        settings
            .sources
            .insert("postgresql.pwd".to_string(), ConfigSource::Cli);
    }
    settings.postgresql = settings.postgresql.clone().merge(server.postgresql);

    // S3 settings
    if server.s3.enabled.is_some() {
        settings
            .sources
            .insert("s3.enabled".to_string(), ConfigSource::Cli);
    }
    if server.s3.access_key.is_some() {
        settings
            .sources
            .insert("s3.access_key".to_string(), ConfigSource::Cli);
    }
    if server.s3.secret_key.is_some() {
        settings
            .sources
            .insert("s3.secret_key".to_string(), ConfigSource::Cli);
    }
    if server.s3.region.is_some() {
        settings
            .sources
            .insert("s3.region".to_string(), ConfigSource::Cli);
    }
    if server.s3.endpoint.is_some() {
        settings
            .sources
            .insert("s3.endpoint".to_string(), ConfigSource::Cli);
    }
    if server.s3.allow_http.is_some() {
        settings
            .sources
            .insert("s3.allow_http".to_string(), ConfigSource::Cli);
    }
    if server.s3.crates_bucket.is_some() {
        settings
            .sources
            .insert("s3.crates_bucket".to_string(), ConfigSource::Cli);
    }
    if server.s3.cratesio_bucket.is_some() {
        settings
            .sources
            .insert("s3.cratesio_bucket".to_string(), ConfigSource::Cli);
    }
    if server.s3.toolchain_bucket.is_some() {
        settings
            .sources
            .insert("s3.toolchain_bucket".to_string(), ConfigSource::Cli);
    }
    settings.s3 = settings.s3.clone().merge(server.s3);

    // Setup settings
    if server.setup.admin_pwd.is_some() {
        settings
            .sources
            .insert("setup.admin_pwd".to_string(), ConfigSource::Cli);
    }
    if server.setup.admin_token.is_some() {
        settings
            .sources
            .insert("setup.admin_token".to_string(), ConfigSource::Cli);
    }
    settings.setup = settings.setup.clone().merge(server.setup);

    // OAuth2 settings
    if server.oauth2.enabled.is_some() {
        settings
            .sources
            .insert("oauth2.enabled".to_string(), ConfigSource::Cli);
    }
    if server.oauth2.issuer_url.is_some() {
        settings
            .sources
            .insert("oauth2.issuer_url".to_string(), ConfigSource::Cli);
    }
    if server.oauth2.client_id.is_some() {
        settings
            .sources
            .insert("oauth2.client_id".to_string(), ConfigSource::Cli);
    }
    if server.oauth2.client_secret.is_some() {
        settings
            .sources
            .insert("oauth2.client_secret".to_string(), ConfigSource::Cli);
    }
    if server.oauth2.scopes.is_some() {
        settings
            .sources
            .insert("oauth2.scopes".to_string(), ConfigSource::Cli);
    }
    if server.oauth2.auto_provision_users.is_some() {
        settings
            .sources
            .insert("oauth2.auto_provision_users".to_string(), ConfigSource::Cli);
    }
    if server.oauth2.admin_group_claim.is_some() {
        settings
            .sources
            .insert("oauth2.admin_group_claim".to_string(), ConfigSource::Cli);
    }
    if server.oauth2.admin_group_value.is_some() {
        settings
            .sources
            .insert("oauth2.admin_group_value".to_string(), ConfigSource::Cli);
    }
    if server.oauth2.read_only_group_claim.is_some() {
        settings.sources.insert(
            "oauth2.read_only_group_claim".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.oauth2.read_only_group_value.is_some() {
        settings.sources.insert(
            "oauth2.read_only_group_value".to_string(),
            ConfigSource::Cli,
        );
    }
    if server.oauth2.button_text.is_some() {
        settings
            .sources
            .insert("oauth2.button_text".to_string(), ConfigSource::Cli);
    }
    settings.oauth2 = settings.oauth2.clone().merge(server.oauth2);

    // Toolchain settings
    if server.toolchain.enabled.is_some() {
        settings
            .sources
            .insert("toolchain.enabled".to_string(), ConfigSource::Cli);
    }
    if server.toolchain.max_size.is_some() {
        settings
            .sources
            .insert("toolchain.max_size".to_string(), ConfigSource::Cli);
    }
    settings.toolchain = settings.toolchain.clone().merge(server.toolchain);

    // Log settings (manual handling since they don't use ClapSerde)
    if let Some(format) = server.log_format {
        settings.log.format = format;
        settings
            .sources
            .insert("log.format".to_string(), ConfigSource::Cli);
    }
    if let Some(level) = server.log_level {
        settings.log.level = level;
        settings
            .sources
            .insert("log.level".to_string(), ConfigSource::Cli);
    }
    if let Some(level) = server.log_level_web_server {
        settings.log.level_web_server = level;
        settings
            .sources
            .insert("log.level_web_server".to_string(), ConfigSource::Cli);
    }
}
