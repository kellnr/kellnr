use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

fn default_pg_address() -> String {
    "localhost".to_string()
}

fn default_pg_db() -> String {
    "kellnr".to_string()
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct Postgresql {
    /// Use `PostgreSQL` instead of `SQLite`
    #[default(false)]
    #[arg(id = "postgresql-enabled", long = "postgresql-enabled")]
    pub enabled: bool,

    /// `PostgreSQL` server address
    #[default(default_pg_address())]
    #[arg(id = "postgresql-address", long = "postgresql-address")]
    pub address: String,

    /// `PostgreSQL` port
    #[default(5432)]
    #[arg(id = "postgresql-port", long = "postgresql-port")]
    pub port: u16,

    /// Database name
    #[default(default_pg_db())]
    #[arg(id = "postgresql-db", long = "postgresql-db")]
    pub db: String,

    /// Database user
    #[default(String::new())]
    #[arg(id = "postgresql-user", long = "postgresql-user")]
    pub user: String,

    /// Database password
    #[default(String::new())]
    #[serde(skip_serializing, default)]
    #[arg(id = "postgresql-pwd", long = "postgresql-pwd")]
    pub pwd: String,
}
