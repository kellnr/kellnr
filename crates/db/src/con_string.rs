use crate::password::generate_salt;
use settings::Settings;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConString {
    Postgres(PgConString),
    Sqlite(SqliteConString),
}

impl Display for ConString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let con_string = match self {
            ConString::Postgres(p) => p.to_string(),
            ConString::Sqlite(s) => s.to_string(),
        };
        write!(f, "{}", con_string)
    }
}

impl ConString {
    pub fn admin_pwd(&self) -> String {
        match self {
            ConString::Postgres(p) => p.admin.pwd.to_string(),
            ConString::Sqlite(s) => s.admin_pwd.to_string(),
        }
    }

    pub fn salt(&self) -> String {
        match self {
            ConString::Postgres(p) => p.admin.salt.to_string(),
            ConString::Sqlite(s) => s.salt.to_string(),
        }
    }

    pub fn admin_token(&self) -> String {
        match self {
            ConString::Postgres(p) => p.admin.token.to_string(),
            ConString::Sqlite(s) => s.admin_token.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdminUser {
    pub pwd: String,
    pub token: String,
    pub salt: String,
}

impl AdminUser {
    pub fn new(pwd: String, token: String, salt: String) -> Self {
        Self { pwd, token, salt }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PgConString {
    addr: String,
    port: u16,
    db: String,
    user: String,
    pwd: String,
    admin: AdminUser,
}

impl PgConString {
    pub fn new(addr: &str, port: u16, db: &str, user: &str, pwd: &str, admin: AdminUser) -> Self {
        Self {
            addr: addr.to_owned(),
            port,
            db: db.to_owned(),
            user: user.to_owned(),
            pwd: pwd.to_owned(),
            admin,
        }
    }
}

impl From<&Settings> for PgConString {
    fn from(s: &Settings) -> Self {
        Self {
            addr: s.postgresql.address.clone(),
            port: s.postgresql.port,
            db: s.postgresql.db.clone(),
            user: s.postgresql.user.clone(),
            pwd: s.postgresql.pwd.clone(),
            admin: AdminUser {
                pwd: s.setup.admin_pwd.clone(),
                token: s.setup.admin_token.clone(),
                salt: generate_salt(),
            },
        }
    }
}

impl Display for PgConString {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.pwd, self.addr, self.port, self.db
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SqliteConString {
    pub path: PathBuf,
    pub salt: String,
    pub admin_pwd: String,
    pub admin_token: String,
    pub session_age: Duration,
}

impl SqliteConString {
    pub fn new(
        path: &Path,
        salt: &str,
        admin_pwd: &str,
        admin_token: &str,
        session_age: Duration,
    ) -> Self {
        Self {
            path: path.to_owned(),
            salt: salt.to_owned(),
            admin_pwd: admin_pwd.to_owned(),
            admin_token: admin_token.to_owned(),
            session_age,
        }
    }
}

impl From<&Settings> for SqliteConString {
    fn from(settings: &Settings) -> Self {
        Self {
            path: settings.sqlite_path(),
            salt: generate_salt(),
            admin_pwd: settings.setup.admin_pwd.to_owned(),
            admin_token: settings.setup.admin_token.to_owned(),
            session_age: Duration::from_secs(settings.registry.session_age_seconds),
        }
    }
}

impl Display for SqliteConString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "sqlite://{}?mode=rwc", self.path.display())
    }
}
