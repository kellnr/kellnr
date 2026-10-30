use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "docs")]
pub struct Docs {
    /// Enable documentation hosting
    ///
    /// Building docs runs the build scripts and proc macros of every
    /// published crate on the Kellnr host. See `build_wrapper` before
    /// enabling this for publishers you do not fully trust.
    pub enabled: bool,

    /// Max docs size in MB
    pub max_size: usize,

    /// Max duration of a single rustdoc build in seconds (0 = unlimited).
    /// Builds that take longer are killed and the crate is skipped.
    pub build_timeout_seconds: u64,

    /// Command prefix (comma-separated) for the rustdoc build process.
    ///
    /// Building docs compiles the published crate, which runs its build
    /// scripts and proc macros. Kellnr runs every build in a child process
    /// with a cleared environment, so no `KELLNR_*` settings or secrets are
    /// passed on. Cargo's own `CARGO_*` variables are kept, so a registry
    /// token in `CARGO_REGISTRIES_<NAME>_TOKEN` is visible to build scripts
    /// and should belong to a dedicated read-only user.
    /// The build still runs as the Kellnr user: it can read every
    /// file Kellnr can read (for example the config file or the `SQLite`
    /// database, e.g. via `include_str!`) and it has network access. Kellnr
    /// only publishes the files rustdoc generates, but the build can write
    /// such content into the generated docs of the crate. Only enable docs
    /// for crates you trust, or set a wrapper that confines the build, e.g.
    /// with bubblewrap:
    /// `["bwrap", "--ro-bind", "/", "/", "--tmpfs", "<data_dir>",
    /// "--bind", "<data_dir>/doc_queue", "<data_dir>/doc_queue",
    /// "--bind", "<cargo_home>", "<cargo_home>", "--tmpfs", "/etc/kellnr",
    /// "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp",
    /// "--unshare-pid", "--die-with-parent", "--"]`.
    /// The wrapper is followed by the Kellnr binary and its arguments.
    #[configurable(env_list)]
    #[arg(value_delimiter = ',')]
    pub build_wrapper: Vec<String>,
}

impl Default for Docs {
    fn default() -> Self {
        Self {
            enabled: false,
            max_size: 100,
            build_timeout_seconds: 1800,
            build_wrapper: Vec::new(),
        }
    }
}
