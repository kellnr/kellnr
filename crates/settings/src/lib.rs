pub mod cli;
pub mod compile_time_config;
mod config_source;
pub mod constants;
mod docs;
mod filter;
pub mod gcs;
mod leaf_labels;
mod local;
mod log;
pub mod oauth2;
mod origin;
mod postgresql;
mod protocol;
pub mod proxy;
mod registry;
pub mod s3;
mod settings;
mod setup;
mod toolchain;
mod webhooks;

pub use cli::{CliResult, ResolvedSettings, ShowConfigOptions, cli_flag_map, parse_cli};
pub use config_source::{ConfigSource, SourceMap};
pub use docs::Docs;
pub use filter::{CratesioFilter, LicenseUnknown, RegistryFilter};
pub use gcs::Gcs;
pub use leaf_labels::leaf_label;
pub use local::Local;
pub use log::{LogFormat, LogLevel};
pub use oauth2::{ClaimMatch, OAuth2};
pub use origin::{Origin, UrlPathPrefix};
pub use postgresql::Postgresql;
pub use protocol::Protocol;
// Re-export the provcfg-generated Prov type and the Provenance trait so
// callers (e.g. the `kellnr config show` printer) can walk per-leaf
// provenance directly without round-tripping through `Settings`. Also
// re-export `erased_serde` so the walk visitor's value parameter can be
// referenced without a direct provcfg dep.
pub use provcfg::{Category, Config, Provenance, erased_serde};
pub use proxy::Proxy;
pub use registry::{Registry, SessionAge};
pub use settings::{
    Settings, SettingsError, SettingsProv, build_prov_with_cli, settings_from_prov,
    sources_from_prov, test_settings,
};
pub use setup::Setup;
pub use toolchain::Toolchain;
pub use webhooks::Webhooks;
