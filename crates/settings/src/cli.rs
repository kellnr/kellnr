use std::path::PathBuf;

use clap::{Parser, Subcommand};
use clap_serde_derive::ClapSerde;
use config::ConfigError;

use crate::compile_time_config;
use crate::docs::Docs;
use crate::local::Local;
use crate::log::{LogFormat, LogLevel};
use crate::origin::Origin;
use crate::postgresql::Postgresql;
use crate::proxy::Proxy;
use crate::registry::Registry;
use crate::s3::S3;
use crate::settings::Settings;
use crate::setup::Setup;

#[derive(Parser)]
#[command(name = "kellnr", version, about)]
pub struct Cli {
    /// Path to configuration file
    #[arg(id = "config", short = 'c', long = "config", global = true)]
    pub config_file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,

    // Server options (used when no subcommand is provided)
    #[command(flatten)]
    pub server: ServerArgs,
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
pub enum Command {
    /// Configuration management commands
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show the current configuration
    Show,
}

/// Result of parsing the CLI
pub enum CliResult {
    /// Run the server with the given settings
    RunServer(Settings),
    /// Show the current configuration
    ShowConfig(Settings),
}

pub fn parse_cli() -> Result<CliResult, ConfigError> {
    let cli = Cli::parse();

    // Config file priority: CLI > env var > compile-time > None
    let env_config_file = std::env::var("KELLNR_CONFIG_FILE").ok();
    let config_file: Option<PathBuf> = cli
        .config_file
        .or(env_config_file.map(PathBuf::from))
        .or(compile_time_config::KELLNR_COMPTIME__CONFIG_FILE.map(PathBuf::from));

    // Load settings from file + env
    let mut settings = Settings::try_from(config_file.as_deref())?;

    // Handle subcommands
    match cli.command {
        Some(Command::Config { action }) => match action {
            ConfigAction::Show => Ok(CliResult::ShowConfig(settings)),
        },
        None => {
            // No subcommand - run server, merge CLI args
            settings.local = Local::from(settings.local).merge(cli.server.local);
            settings.origin = Origin::from(settings.origin).merge(cli.server.origin);
            settings.registry = Registry::from(settings.registry).merge(cli.server.registry);
            settings.docs = Docs::from(settings.docs).merge(cli.server.docs);
            settings.proxy = Proxy::from(settings.proxy).merge(cli.server.proxy);
            settings.postgresql = Postgresql::from(settings.postgresql).merge(cli.server.postgresql);
            settings.s3 = S3::from(settings.s3).merge(cli.server.s3);
            settings.setup = Setup::from(settings.setup).merge(cli.server.setup);

            // Log settings (manual merge due to custom serde deserializers)
            if let Some(format) = cli.server.log_format {
                settings.log.format = format;
            }
            if let Some(level) = cli.server.log_level {
                settings.log.level = level;
            }
            if let Some(level) = cli.server.log_level_web_server {
                settings.log.level_web_server = level;
            }

            Ok(CliResult::RunServer(settings))
        }
    }
}

/// Legacy function for backward compatibility
pub fn get_settings_with_cli() -> Result<Settings, ConfigError> {
    match parse_cli()? {
        CliResult::RunServer(settings) | CliResult::ShowConfig(settings) => Ok(settings),
    }
}
