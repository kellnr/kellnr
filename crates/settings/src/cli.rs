use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use clap_serde_derive::ClapSerde;
use config::ConfigError;

use crate::compile_time_config;
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
    /// Run the kellnr server
    Run {
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
    Show,
    /// Initialize a new configuration file with default values
    Init {
        /// Output file path (default: ./kellnr.toml)
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,
    },
}

/// Result of parsing the CLI
pub enum CliResult {
    /// Run the server with the given settings
    RunServer(Settings),
    /// Show the current configuration
    ShowConfig(Settings),
    /// Initialize a new configuration file
    InitConfig {
        /// Default settings to write
        settings: Settings,
        /// Output file path
        output: PathBuf,
    },
    /// Show help/usage information
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

    // Load settings from file + env
    let mut settings = Settings::try_from(config_file.as_deref())?;

    // Handle subcommands
    match cli.command {
        Some(Command::Run { server }) => {
            // Run server, merge CLI args
            settings.local = settings.local.merge(server.local);
            settings.origin = settings.origin.merge(server.origin);
            settings.registry = settings.registry.merge(server.registry);
            settings.docs = settings.docs.merge(server.docs);
            settings.proxy = settings.proxy.merge(server.proxy);
            settings.postgresql = settings.postgresql.merge(server.postgresql);
            settings.s3 = settings.s3.merge(server.s3);
            settings.setup = settings.setup.merge(server.setup);
            settings.oauth2 = settings.oauth2.merge(server.oauth2);

            // Log settings (manual merge due to custom serde deserializers)
            if let Some(format) = server.log_format {
                settings.log.format = format;
            }
            if let Some(level) = server.log_level {
                settings.log.level = level;
            }
            if let Some(level) = server.log_level_web_server {
                settings.log.level_web_server = level;
            }

            Ok(CliResult::RunServer(settings))
        }
        Some(Command::Config { action }) => match action {
            ConfigAction::Show => Ok(CliResult::ShowConfig(settings)),
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
        | CliResult::ShowConfig(settings)
        | CliResult::InitConfig { settings, .. } => Ok(settings),
        CliResult::ShowHelp => Ok(Settings::default()),
    }
}
