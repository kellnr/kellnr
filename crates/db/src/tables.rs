use kellnr_migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbErr};
use tracing::{debug, info};

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

    // Clean up old migration entries before running migrations.
    // This is necessary for upgrading from v5.14.0 or earlier to v6.0.0.
    // SeaORM's Migrator::up() validates that all applied migrations have
    // corresponding migration files, but we've consolidated the old migrations.
    cleanup_old_migrations(&db).await?;

    Migrator::up(&db, None).await?;
    Ok(db)
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
            debug!("Removed old migration entry: {}", migration);
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
