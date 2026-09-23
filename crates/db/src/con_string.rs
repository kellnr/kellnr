use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::time::Duration;

use kellnr_settings::Settings;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

use crate::password::generate_salt;

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
        write!(f, "{con_string}")
    }
}

impl ConString {
    pub fn admin_pwd(&self) -> String {
        match self {
            ConString::Postgres(p) => p.admin.pwd.clone(),
            ConString::Sqlite(s) => s.admin_pwd.clone(),
        }
    }

    pub fn salt(&self) -> String {
        match self {
            ConString::Postgres(p) => p.admin.salt.clone(),
            ConString::Sqlite(s) => s.salt.clone(),
        }
    }

    pub fn admin_token(&self) -> Option<String> {
        match self {
            ConString::Postgres(p) => p.admin.token.clone(),
            ConString::Sqlite(s) => s.admin_token.clone(),
        }
    }

    /// Maximum lifetime of a session before it is considered expired.
    pub fn session_age(&self) -> Duration {
        match self {
            ConString::Postgres(p) => p.session_age,
            ConString::Sqlite(s) => s.session_age,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdminUser {
    pub pwd: String,
    pub token: Option<String>,
    pub salt: String,
}

impl AdminUser {
    pub fn new(pwd: String, token: Option<String>, salt: String) -> Self {
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
    pub session_age: Duration,
}

impl PgConString {
    pub fn new(
        addr: &str,
        port: u16,
        db: &str,
        user: &str,
        pwd: &str,
        admin: AdminUser,
        session_age: Duration,
    ) -> Self {
        Self {
            addr: addr.to_owned(),
            port,
            db: db.to_owned(),
            user: user.to_owned(),
            pwd: pwd.to_owned(),
            admin,
            session_age,
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
            session_age: Duration::from_secs(s.registry.session_age_seconds),
        }
    }
}

/// Characters that must be percent-encoded in the user, password and database
/// parts of the connection URL. Everything except the RFC 3986 unreserved characters.
const URL_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

impl Display for PgConString {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "postgres://{}:{}@{}:{}/{}",
            utf8_percent_encode(&self.user, URL_COMPONENT),
            utf8_percent_encode(&self.pwd, URL_COMPONENT),
            self.addr,
            self.port,
            utf8_percent_encode(&self.db, URL_COMPONENT)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SqliteConString {
    pub path: PathBuf,
    pub salt: String,
    pub admin_pwd: String,
    pub admin_token: Option<String>,
    pub session_age: Duration,
}

impl SqliteConString {
    pub fn new(
        path: &Path,
        salt: &str,
        admin_pwd: &str,
        admin_token: Option<String>,
        session_age: Duration,
    ) -> Self {
        Self {
            path: path.to_owned(),
            salt: salt.to_owned(),
            admin_pwd: admin_pwd.to_owned(),
            admin_token,
            session_age,
        }
    }
}

impl From<&Settings> for SqliteConString {
    fn from(settings: &Settings) -> Self {
        Self {
            path: settings.sqlite_path(),
            salt: generate_salt(),
            admin_pwd: settings.setup.admin_pwd.clone(),
            admin_token: settings.setup.admin_token.clone(),
            session_age: Duration::from_secs(settings.registry.session_age_seconds),
        }
    }
}

impl Display for SqliteConString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.path.to_str() == Some(":memory:") {
            write!(f, "sqlite::memory:")
        } else {
            write!(f, "sqlite://{}?mode=rwc", self.path.display())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use percent_encoding::percent_decode_str;
    use sea_orm::sqlx::postgres::PgConnectOptions;
    use url::Url;

    use super::*;

    fn pg_con_string(user: &str, pwd: &str, db: &str) -> PgConString {
        PgConString::new(
            "localhost",
            5432,
            db,
            user,
            pwd,
            AdminUser::new("admin".to_string(), None, "salt".to_string()),
            Duration::from_mins(1),
        )
    }

    #[test]
    fn pg_con_string_plain_credentials_are_unchanged() {
        let con = pg_con_string("kellnr", "secret", "kellnr-db");

        assert_eq!(
            con.to_string(),
            "postgres://kellnr:secret@localhost:5432/kellnr-db"
        );
    }

    #[test]
    fn pg_con_string_encodes_special_characters() {
        let con = pg_con_string("us@er", "a/b@c:d%e#f?g", "my db");

        assert_eq!(
            con.to_string(),
            "postgres://us%40er:a%2Fb%40c%3Ad%25e%23f%3Fg@localhost:5432/my%20db"
        );
    }

    #[test]
    fn pg_con_string_round_trips_through_sqlx() {
        let user = "us@er:name";
        let pwd = "cDlJH15F/Xj@:%2F#?&=+ äö";
        let db = "kellnr/db";
        let con = pg_con_string(user, pwd, db);

        let opts = PgConnectOptions::from_str(&con.to_string()).unwrap();
        // sqlx has no getter for the password, so decode it the same way sqlx does.
        let url = Url::parse(&con.to_string()).unwrap();
        let decoded_pwd = percent_decode_str(url.password().unwrap())
            .decode_utf8()
            .unwrap();

        assert_eq!(opts.get_username(), user);
        assert_eq!(decoded_pwd, pwd);
        assert_eq!(opts.get_host(), "localhost");
        assert_eq!(opts.get_port(), 5432);
        assert_eq!(opts.get_database(), Some(db));
    }
}
