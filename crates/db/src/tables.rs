use kellnr_migration::{Migrator, MigratorTrait};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DbErr,
    Statement,
};
use tracing::{info, trace};

/// Old migration names from v5.14.0 and earlier that need to be cleaned up
/// before running the v6.0.0 migrations.
const OLD_MIGRATIONS: &[&str] = &[
    "m20220101_000001_create_table",
    "m20220101_000002_create_table",
    "m20220101_000003_create_table",
    "m20220101_000004_create_table",
    "m20220101_000005_create_table",
    "m20220101_000006_create_table",
    "m20220101_000007_create_table",
    "m20220101_000008_create_table",
    "m20220101_000009_create_table",
    "m20220101_0000010_create_table",
    "m20220101_0000011_create_table",
    "m20250227_005754_add_readonly_user",
    "m20250319_191043_add_groups",
    "m20250412_0000012_hash_tokens",
    "m20250414_102510_add_unique_indices",
    "m20250911_000001_cratesio_indices",
    "m20250923_095440_webhooks",
    "m20260122_000001_add_pubtime",
];

pub async fn init_database(
    connection_string: impl ToString,
    max_con: u32,
) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(connection_string.to_string());
    if max_con > 0 {
        opt.max_connections(max_con);
    }
    let db = Database::connect(opt).await?;

    // Validate the schema is ready for v6 BEFORE cleaning up old migrations.
    // This ensures we don't corrupt the migration history if the upgrade
    // requirements aren't met (e.g. user skipped v5.14.0).
    validate_pre_upgrade(&db).await?;

    // Clean up old migration entries before running migrations.
    // This is necessary for upgrading from v5.14.0 or earlier to v6.0.0.
    // SeaORM's Migrator::up() validates that all applied migrations have
    // corresponding migration files, but we've consolidated the old migrations.
    cleanup_old_migrations(&db).await?;

    Migrator::up(&db, None).await?;
    Ok(db)
}

/// Validate that the database schema is ready for the v6 upgrade.
/// This runs BEFORE `cleanup_old_migrations` to ensure we don't destroy
/// the migration history when the schema isn't at v5.14.0.
async fn validate_pre_upgrade(db: &DatabaseConnection) -> Result<(), DbErr> {
    // Check if any old v5 migration entries exist.
    // If not, this is either a fresh install or already upgraded — skip.
    if !has_old_migration_entries(db).await? {
        return Ok(());
    }

    trace!("Detected v5 migration entries, validating schema before upgrade...");

    let column_checks = [
        ("crate_index", "pubtime"),
        ("cratesio_index", "pubtime"),
        ("user", "is_read_only"),
        ("krate", "restricted_download"),
        ("krate", "e_tag"),
        ("krate", "original_name"),
        ("cratesio_crate", "max_version"),
        ("cratesio_meta", "documentation"),
    ];

    for (table, column) in column_checks {
        if !has_column(db, table, column).await? {
            return Err(DbErr::Custom(format!(
                "Missing column '{table}.{column}'. Please ensure you have first upgraded \
                to Kellnr v5.14.0 before upgrading to v6.0.0."
            )));
        }
    }

    Ok(())
}

/// Check if any old v5 migration entries exist in `seaql_migrations`.
async fn has_old_migration_entries(db: &DatabaseConnection) -> Result<bool, DbErr> {
    let backend = db.get_database_backend();
    let stmt = Statement::from_string(
        backend,
        "SELECT version FROM seaql_migrations WHERE version = 'm20220101_000001_create_table' LIMIT 1",
    );
    match db.query_one_raw(stmt).await {
        Ok(Some(_)) => Ok(true),
        Ok(None) | Err(_) => Ok(false),
    }
}

/// Check if a column exists in a table using a database-specific query.
async fn has_column(db: &DatabaseConnection, table: &str, column: &str) -> Result<bool, DbErr> {
    let backend = db.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Postgres => format!(
            "SELECT 1 FROM information_schema.columns \
            WHERE table_name = '{table}' AND column_name = '{column}'"
        ),
        DatabaseBackend::Sqlite => {
            format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = '{column}'")
        }
        _ => return Err(DbErr::Custom("Unsupported database backend".to_string())),
    };
    let stmt = Statement::from_string(backend, sql);
    match db.query_one_raw(stmt).await {
        Ok(Some(_)) => Ok(true),
        Ok(None) | Err(_) => Ok(false),
    }
}

/// Delete old migration entries from `seaql_migrations` table if they exist.
/// This allows the v6.0.0 migrations to run even when upgrading from older versions.
async fn cleanup_old_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    // Just try to delete old migrations. If the table doesn't exist (fresh install),
    // the DELETE will fail silently - that's fine.
    let mut cleaned = 0;
    for migration in OLD_MIGRATIONS {
        let sql = format!("DELETE FROM seaql_migrations WHERE version = '{migration}'");
        if let Ok(result) = db.execute_unprepared(&sql).await
            && result.rows_affected() > 0
        {
            cleaned += 1;
        }
    }

    if cleaned > 0 {
        info!(
            "Cleaned up {} old migration entries for v6.0.0 upgrade",
            cleaned
        );
    }

    Ok(())
}
