use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

fn default_pg_address() -> String {
    "localhost".to_string()
}

fn default_pg_db() -> String {
    "kellnr".to_string()
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "postgresql")]
pub struct Postgresql {
    /// Use `PostgreSQL` instead of `SQLite`
    pub enabled: bool,

    /// `PostgreSQL` server address
    pub address: String,

    /// `PostgreSQL` port
    pub port: u16,

    /// Database name
    pub db: String,

    /// Database user
    pub user: String,

    /// Database password
    #[serde(skip_serializing, default)]
    #[configurable(secret)]
    pub pwd: String,

    /// Raw connection string ("postgres://..."); if present, used instead of the other fields.
    #[serde(skip_serializing, default)]
    #[configurable(secret)]
    pub connection_string: Option<String>,
}

impl Default for Postgresql {
    fn default() -> Self {
        Self {
            enabled: false,
            address: default_pg_address(),
            port: 5432,
            db: default_pg_db(),
            user: String::new(),
            pwd: String::new(),
            connection_string: None,
        }
    }
}
