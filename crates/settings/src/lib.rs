pub mod cli;
pub mod compile_time_config;
pub mod config_source;
pub mod constants;
pub mod docs;
pub mod leaf_labels;
pub mod local;
pub mod log;
pub mod oauth2;
pub mod origin;
pub mod postgresql;
pub mod protocol;
pub mod proxy;
pub mod registry;
pub mod s3;
pub mod settings;
pub mod setup;
pub mod toolchain;

pub use cli::{CliResult, ResolvedSettings, ShowConfigOptions, cli_flag_map, parse_cli};
pub use config_source::{ConfigSource, SourceMap};
pub use docs::Docs;
pub use leaf_labels::leaf_label;
pub use local::Local;
pub use log::{LogFormat, LogLevel};
pub use oauth2::OAuth2;
pub use origin::Origin;
pub use postgresql::Postgresql;
pub use protocol::Protocol;
// Re-export the provcfg-generated Prov type and the Provenance trait so
// callers (e.g. the `kellnr config show` printer) can walk per-leaf
// provenance directly without round-tripping through `Settings`. Also
// re-export `erased_serde` so the walk visitor's value parameter can be
// referenced without a direct provcfg dep.
pub use provcfg::{Category, Config, Provenance, erased_serde};
pub use proxy::Proxy;
pub use registry::Registry;
pub use settings::{
    Settings, SettingsError, SettingsProv, build_prov_with_cli, sources_from_prov, test_settings,
};
pub use setup::Setup;
pub use toolchain::Toolchain;
