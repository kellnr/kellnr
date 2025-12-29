// Environment variable based compile time configuration
// These can be set at compile time to change default paths and version info
// If not set, default values are used.
// Some of the values here can be overridden at runtime via configuration files or environment
// variables. Have a look at the documentation for more information.

pub const KELLNR_DATA_DIR: &str = match option_env!("KELLNR_DATA_DIR") {
    Some(v) => v,
    None => "/opt/kdata",
};

pub const KELLNR_CONFIG_DIR: &str = match option_env!("KELLNR_CONFIG_DIR") {
    Some(v) => v,
    None => "./config",
};

pub const KELLNR_VERSION: &str = match option_env!("KELLNR_VERSION") {
    Some(v) => v,
    None => "0.0.0-unknown",
};
