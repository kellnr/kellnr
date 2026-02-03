use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use clap_serde_derive::ClapSerde;
use config::ConfigError;
use toml::Value;

use crate::SourceMap;
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

/// Compare two Settings structs and mark all differences with the given source.
///
/// Uses serde serialization to automatically iterate over all sections and fields.
/// New sections or fields added to Settings are automatically tracked without
/// requiring manual updates to this function.
fn track_settings_differences(
    sources: &mut SourceMap,
    current: &Settings,
    previous: &Settings,
    source: ConfigSource,
) {
    let Ok(current_value) = Value::try_from(current) else {
        return;
    };
    let Ok(previous_value) = Value::try_from(previous) else {
        return;
    };

    let (Value::Table(current_table), Value::Table(previous_table)) =
        (current_value, previous_value)
    else {
        return;
    };

    for (section_name, section_value) in &current_table {
        // Get the corresponding section from previous settings
        let Some(previous_section) = previous_table.get(section_name) else {
            continue;
        };

        // Both must be tables (config sections)
        let (Value::Table(current_fields), Value::Table(previous_fields)) =
            (section_value, previous_section)
        else {
            continue;
        };

        // Compare each field in the section
        for (field_name, field_value) in current_fields {
            if let Some(previous_field) = previous_fields.get(field_name)
                && field_value != previous_field
            {
                sources.insert(format!("{section_name}.{field_name}"), source);
            }
        }
    }
}

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
///
/// Uses serde serialization to automatically iterate over all sections and fields,
/// so new settings are automatically tracked without manual updates.
fn track_toml_sources(settings: &mut Settings) {
    let current = settings.clone();
    let defaults = Settings::default();
    track_settings_differences(
        &mut settings.sources,
        &current,
        &defaults,
        ConfigSource::Toml,
    );
}

/// Track CLI arguments and merge them into settings.
///
/// Uses a snapshot-and-compare approach: captures settings before merge,
/// performs all merges, then marks any changed fields as CLI-sourced.
/// This ensures new fields are automatically tracked without manual updates.
///
/// Note: New sections still need to be added to the merge list below,
/// but tracking of which fields changed is fully automatic via serde.
fn track_and_merge_cli(settings: &mut Settings, server: ServerArgs) {
    // Snapshot before any merges
    let before = settings.clone();

    // Merge all ClapSerde sections
    settings.local = settings.local.clone().merge(server.local);
    settings.origin = settings.origin.clone().merge(server.origin);
    settings.registry = settings.registry.clone().merge(server.registry);
    settings.docs = settings.docs.clone().merge(server.docs);
    settings.proxy = settings.proxy.clone().merge(server.proxy);
    settings.postgresql = settings.postgresql.clone().merge(server.postgresql);
    settings.s3 = settings.s3.clone().merge(server.s3);
    settings.setup = settings.setup.clone().merge(server.setup);
    settings.oauth2 = settings.oauth2.clone().merge(server.oauth2);
    settings.toolchain = settings.toolchain.clone().merge(server.toolchain);

    // Log settings (manual merge since they don't use ClapSerde)
    if let Some(format) = server.log_format {
        settings.log.format = format;
    }
    if let Some(level) = server.log_level {
        settings.log.level = level;
    }
    if let Some(level) = server.log_level_web_server {
        settings.log.level_web_server = level;
    }

    // Track all changes at once - fully automatic via serde
    // Clone current state to avoid borrow conflict with settings.sources
    let current = settings.clone();
    track_settings_differences(&mut settings.sources, &current, &before, ConfigSource::Cli);
}
