use std::path::PathBuf;

use clap::Parser;
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
    #[arg(id = "config", short = 'c', long = "config")]
    pub config_file: Option<PathBuf>,

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

pub fn get_settings_with_cli() -> Result<Settings, ConfigError> {
    let cli = Cli::parse();

    // Config file priority: CLI > env var > compile-time > None
    let env_config_file = std::env::var("KELLNR_CONFIG_FILE").ok();
    let config_file: Option<PathBuf> = cli
        .config_file
        .or(env_config_file.map(PathBuf::from))
        .or(compile_time_config::KELLNR_COMPTIME__CONFIG_FILE.map(PathBuf::from));

    // Load settings from file + env
    let mut settings = Settings::try_from(config_file.as_deref())?;

    // Merge CLI args (highest priority)
    settings.local = Local::from(settings.local).merge(cli.local);
    settings.origin = Origin::from(settings.origin).merge(cli.origin);
    settings.registry = Registry::from(settings.registry).merge(cli.registry);
    settings.docs = Docs::from(settings.docs).merge(cli.docs);
    settings.proxy = Proxy::from(settings.proxy).merge(cli.proxy);
    settings.postgresql = Postgresql::from(settings.postgresql).merge(cli.postgresql);
    settings.s3 = S3::from(settings.s3).merge(cli.s3);
    settings.setup = Setup::from(settings.setup).merge(cli.setup);

    // Log settings (manual merge due to custom serde deserializers)
    if let Some(format) = cli.log_format {
        settings.log.format = format;
    }
    if let Some(level) = cli.log_level {
        settings.log.level = level;
    }
    if let Some(level) = cli.log_level_web_server {
        settings.log.level_web_server = level;
    }

    Ok(settings)
}
