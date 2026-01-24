// Environment variable based compile time configuration
// These can be set at compile time to change default paths and version info
// If not set, default values are used.
// Some of the values here can be overridden at runtime via configuration files or environment
// variables. Have a look at the documentation for more information.

pub const KELLNR_COMPTIME__DATA_DIR: &str = match option_env!("KELLNR_COMPTIME__DATA_DIR") {
    Some(v) => v,
    None => "/opt/kdata",
};

pub const KELLNR_COMPTIME__CONFIG_FILE: Option<&str> = option_env!("KELLNR_COMPTIME__CONFIG_FILE");

pub const KELLNR_COMPTIME__VERSION: &str = match option_env!("KELLNR_COMPTIME__VERSION") {
    Some(v) => v,
    None => "0.0.0-unknown",
};
