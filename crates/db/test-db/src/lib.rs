//! Fresh, fully migrated databases for tests.
//!
//! Running all migrations and hashing the admin password for every test is
//! slow. Instead, the first test of a test run creates a template database with
//! [`Database::new`] and every test gets its own copy of it. The test then
//! opens its copy with [`Database::new`] as well, like a restart of Kellnr
//! with an up to date schema. SQLite and PostgreSQL work the same way.
//!
//! Templates live as long as one test run. nextest runs every test in its own
//! process and gives all of them the same `NEXTEST_RUN_ID`, so the first
//! process creates the template under a lock and all others copy it. Without
//! nextest, every test process creates its own template.
//!
//! The PostgreSQL tests use the server at `KELLNR_TEST_PG_ADDR` (`host:port`,
//! user and password `admin`, database `kellnr`), which `just test-docker`
//! starts once for all tests. Without it, every test starts its own container.

mod image;

use std::fs::{self, File};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};

use kellnr_db::{AdminUser, ConString, Database, DbProvider, PgConString, SqliteConString};
use kellnr_settings::{SessionAge, Settings};
use sea_orm::{ConnectOptions, ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use tempfile::TempDir;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;

/// Environment variable with the `host:port` of a PostgreSQL server for tests.
pub const PG_ADDR_ENV: &str = "KELLNR_TEST_PG_ADDR";

const ADMIN_PWD: &str = "123";
const ADMIN_TOKEN: &str = "token";
const MAX_CON: u32 = 10;

/// Template directories of earlier runs older than this are removed.
const STALE_RUN_AGE: Duration = Duration::from_hours(24);

/// A fresh database for one test, removed when dropped.
pub struct TestDb {
    // Dropped first, so the connections are closed before the database is removed.
    db: Arc<Database>,
    _cleanup: Cleanup,
}

impl Deref for TestDb {
    type Target = Database;

    fn deref(&self) -> &Database {
        &self.db
    }
}

impl TestDb {
    /// The database as Kellnr's components take it, e.g. for `AppStateData`.
    pub fn provider(&self) -> Arc<dyn DbProvider> {
        self.db.clone()
    }

    /// A copy of the SQLite template in its own temporary directory.
    pub async fn sqlite() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("kellnr-test-db-")
            .tempdir()
            .expect("cannot create temporary directory for test database");
        let con = ConString::Sqlite(SqliteConString::new(
            &dir.path().join("db.sqlite"),
            ADMIN_PWD,
            Some(ADMIN_TOKEN.to_string()),
            session_age(),
        ));
        copy_sqlite_template(&con).await;
        let db = Database::new(&con, MAX_CON)
            .await
            .expect("cannot open SQLite test database");
        Self {
            db: Arc::new(db),
            _cleanup: Cleanup::Sqlite { _dir: dir },
        }
    }

    /// A copy of the PostgreSQL template on the server from [`PG_ADDR_ENV`],
    /// or on a new container if the variable is not set.
    pub async fn postgres() -> Self {
        let (host, port, container) = if let Ok(addr) = std::env::var(PG_ADDR_ENV) {
            let (host, port) = addr
                .rsplit_once(':')
                .unwrap_or_else(|| panic!("{PG_ADDR_ENV} must be host:port, got {addr}"));
            let port = port
                .parse()
                .unwrap_or_else(|_| panic!("invalid port in {PG_ADDR_ENV}: {addr}"));
            (host.to_string(), port, None)
        } else {
            let container = image::Postgres::default()
                .start()
                .await
                .expect("cannot start postgres container");
            let port = container
                .get_host_port_ipv4(image::Postgres::PG_PORT)
                .await
                .expect("cannot get postgres port");
            ("localhost".to_string(), port, Some(Box::new(container)))
        };
        let server = PgServer { host, port };

        let template = server.template().await;
        let name = format!("kellnr_test_{}", random_suffix());
        let admin = server.connect(image::POSTGRES_DB).await;
        // Several tests copy the template at the same time, which PostgreSQL
        // allows as long as nobody is connected to the template.
        admin
            .execute_unprepared(&format!(
                "CREATE DATABASE \"{name}\" TEMPLATE \"{template}\""
            ))
            .await
            .expect("cannot create test database from template");
        admin.close().await.ok();

        let db = Database::new(&server.con_string(&name), MAX_CON)
            .await
            .expect("cannot open PostgreSQL test database");
        Self {
            db: Arc::new(db),
            _cleanup: Cleanup::Postgres {
                server,
                name,
                _container: container,
            },
        }
    }
}

enum Cleanup {
    Sqlite {
        _dir: TempDir,
    },
    Postgres {
        server: PgServer,
        name: String,
        // The container of this test only, removed when dropped.
        _container: Option<Box<ContainerAsync<image::Postgres>>>,
    },
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let Cleanup::Postgres {
            server,
            name,
            _container: None,
        } = self
        else {
            // The temporary directory and the own container remove themselves.
            return;
        };
        // A shared server lives for the whole run, so drop the database to
        // keep its disk usage low. Drop cannot be async and the test runtime
        // cannot be blocked from within, so use a runtime on another thread.
        let server = server.clone();
        let name = name.clone();
        let _ = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("cannot create runtime")
                .block_on(async {
                    let admin = server.connect(image::POSTGRES_DB).await;
                    let _ = admin
                        .execute_unprepared(&format!(
                            "DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"
                        ))
                        .await;
                    admin.close().await.ok();
                });
        })
        .join();
    }
}

/// Opens a copy of the SQLite template at the database path of `settings`,
/// for tests that need the database in the data directory of Kellnr.
pub async fn sqlite_database(settings: &Settings) -> Database {
    let con = ConString::Sqlite(SqliteConString::from(settings));
    copy_sqlite_template(&con).await;
    Database::new(&con, MAX_CON)
        .await
        .expect("cannot open SQLite test database")
}

/// Puts a copy of the SQLite template for the admin credentials of `con` at
/// the database path of `con`, so the next [`Database::new`] finds a migrated
/// database with the admin user. An existing database is left as it is.
pub async fn copy_sqlite_template(con: &ConString) {
    let ConString::Sqlite(sqlite) = con else {
        panic!("copy_sqlite_template needs a SQLite connection string");
    };
    if sqlite.path.exists() {
        return;
    }
    let template = sqlite_template(sqlite).await;
    fs::copy(&template, &sqlite.path).unwrap_or_else(|e| {
        panic!(
            "cannot copy SQLite template to {}: {e}",
            sqlite.path.display()
        )
    });
}

/// Tasks of this process wait for the SQLite template here without blocking a
/// runtime thread, so only one of them takes the file lock that other
/// processes wait on.
static BUILDING_SQLITE_TEMPLATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Path of the SQLite template for the admin credentials of `con`, created on
/// first use in this test run.
async fn sqlite_template(con: &SqliteConString) -> PathBuf {
    let key = admin_key(&con.admin_pwd, con.admin_token.as_deref());
    let dir = run_dir();
    let template = dir.join(format!("{key}.sqlite"));
    // The template is renamed into place when complete, so it is ready if it exists.
    if template.exists() {
        return template;
    }

    let _building = BUILDING_SQLITE_TEMPLATE.lock().await;
    if template.exists() {
        return template;
    }
    let lock = File::create(dir.join(format!("{key}.lock"))).expect("cannot create lock file");
    lock.lock().expect("cannot lock SQLite template");
    if template.exists() {
        return template;
    }

    let build = dir.join(format!("{key}.build.sqlite"));
    remove_sqlite_files(&build);
    let build_con = ConString::Sqlite(SqliteConString::new(
        &build,
        &con.admin_pwd,
        con.admin_token.clone(),
        con.session_age,
    ));
    drop(
        Database::new(&build_con, 1)
            .await
            .expect("cannot create SQLite template"),
    );

    // Unlike a plain file copy, `VACUUM INTO` writes a single consistent file
    // that also holds the changes still in the write-ahead log.
    let partial = dir.join(format!("{key}.partial.sqlite"));
    remove_sqlite_files(&partial);
    let source = sea_orm::Database::connect(format!("sqlite://{}?mode=ro", build.display()))
        .await
        .expect("cannot open SQLite template");
    let target = partial.to_string_lossy().replace('\'', "''");
    source
        .execute_unprepared(&format!("VACUUM INTO '{target}'"))
        .await
        .expect("cannot write SQLite template");
    source.close().await.ok();
    fs::rename(&partial, &template).expect("cannot move SQLite template into place");
    remove_sqlite_files(&build);
    template
}

fn remove_sqlite_files(path: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut file = path.as_os_str().to_owned();
        file.push(suffix);
        let _ = fs::remove_file(file);
    }
}

/// Directory for the SQLite templates of this test run.
fn run_dir() -> PathBuf {
    let base = std::env::temp_dir().join("kellnr-test-db-templates");
    let dir = base.join(run_key());
    if !dir.exists() {
        remove_stale_run_dirs(&base);
        fs::create_dir_all(&dir).expect("cannot create directory for SQLite templates");
    }
    dir
}

/// Removes the templates of runs that ended long ago, as no run knows when it ends.
fn remove_stale_run_dirs(base: &Path) {
    let Ok(entries) = fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > STALE_RUN_AGE);
        if stale {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Identifies the current test run: the nextest run, or this process otherwise.
fn run_key() -> &'static str {
    static KEY: OnceLock<String> = OnceLock::new();
    KEY.get_or_init(|| match std::env::var("NEXTEST_RUN_ID") {
        Ok(id) => id
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_lowercase(),
        Err(_) => format!("p{}", random_suffix()),
    })
}

/// Templates differ by the admin credentials they were created with.
fn admin_key(pwd: &str, token: Option<&str>) -> String {
    let digest = sha256::digest(format!("{pwd}\n{}", token.unwrap_or_default()));
    digest[..12].to_string()
}

fn random_suffix() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn session_age() -> SessionAge {
    SessionAge::from_secs(3600)
}

#[derive(Clone)]
struct PgServer {
    host: String,
    port: u16,
}

impl PgServer {
    fn con_string(&self, db: &str) -> ConString {
        ConString::Postgres(PgConString::new(
            &self.host,
            self.port,
            db,
            image::POSTGRES_USER,
            image::POSTGRES_PASSWORD,
            AdminUser::new(ADMIN_PWD.to_string(), Some(ADMIN_TOKEN.to_string())),
            session_age(),
        ))
    }

    /// A single connection, so session level advisory locks stay on it.
    async fn connect(&self, db: &str) -> DatabaseConnection {
        let mut opt = ConnectOptions::new(self.con_string(db).to_string());
        opt.max_connections(1).sqlx_logging(false);
        sea_orm::Database::connect(opt)
            .await
            .expect("cannot connect to postgres")
    }

    /// Name of the template database of this test run, created on first use.
    async fn template(&self) -> String {
        let template = format!("kellnr_template_{}", run_key());
        let admin = self.connect(image::POSTGRES_DB).await;
        admin
            .execute_unprepared("SELECT pg_advisory_lock(4711)")
            .await
            .expect("cannot lock PostgreSQL template");

        if !database_exists(&admin, &template).await {
            let build = format!("{template}_build");
            admin
                .execute_unprepared(&format!("DROP DATABASE IF EXISTS \"{build}\" WITH (FORCE)"))
                .await
                .expect("cannot remove old PostgreSQL template");
            admin
                .execute_unprepared(&format!("CREATE DATABASE \"{build}\""))
                .await
                .expect("cannot create PostgreSQL template");
            drop(
                Database::new(&self.con_string(&build), 1)
                    .await
                    .expect("cannot migrate PostgreSQL template"),
            );
            // Nobody may be connected to a template that gets copied. The
            // dropped pool might not have closed its connection yet, and
            // renaming fails as long as it is open.
            admin
                .execute_unprepared(&format!(
                    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{build}'"
                ))
                .await
                .expect("cannot close connections to PostgreSQL template");
            let mut attempts = 0;
            while let Err(e) = admin
                .execute_unprepared(&format!(
                    "ALTER DATABASE \"{build}\" RENAME TO \"{template}\""
                ))
                .await
            {
                attempts += 1;
                assert!(attempts < 100, "cannot rename PostgreSQL template: {e}");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            admin
                .execute_unprepared(&format!(
                    "ALTER DATABASE \"{template}\" WITH ALLOW_CONNECTIONS false"
                ))
                .await
                .expect("cannot protect PostgreSQL template");
        }

        admin
            .execute_unprepared("SELECT pg_advisory_unlock(4711)")
            .await
            .expect("cannot unlock PostgreSQL template");
        admin.close().await.ok();
        template
    }
}

async fn database_exists(con: &DatabaseConnection, name: &str) -> bool {
    con.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT 1 FROM pg_database WHERE datname = $1",
        [name.into()],
    ))
    .await
    .expect("cannot query PostgreSQL databases")
    .is_some()
}
