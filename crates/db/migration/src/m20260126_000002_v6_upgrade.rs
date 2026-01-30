//! Upgrade migration for Kellnr v6.0.0
//!
//! This migration handles upgrades from v5.14.0 by:
//! 1. Validating that the schema is complete (all tables and indices exist)
//! 2. Cleaning up old migration entries from seaql_migrations
//!
//! For fresh installs, the baseline migration creates all tables with
//! `if_not_exists`, so this migration simply validates and cleans up.
//!
//! Users MUST upgrade to v5.14.0 before upgrading to v6.0.0.

use sea_orm_migration::prelude::*;
use tracing::{info, warn};

/// Old migration names that were used in v5.14.0 and earlier
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

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Validate the schema is complete
        // This ensures the database is either:
        // - A fresh install (baseline just ran and created all tables)
        // - An upgrade from v5.14.0 (tables already exist)
        validate_schema(manager).await?;

        // Clean up old migration entries from seaql_migrations
        // This is a no-op for fresh installs (no old entries to delete)
        cleanup_old_migrations(manager).await?;

        info!("Kellnr v6.0.0 migration complete");
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // This migration cannot be meaningfully reversed
        // The old migration entries cannot be restored
        warn!("The v6 upgrade migration cannot be reversed. The database schema remains intact.");
        Ok(())
    }
}

/// Validate that the schema is complete
async fn validate_schema(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // Check for key tables that should exist
    let required_tables = [
        "user",
        "auth_token",
        "session",
        "group",
        "group_user",
        "krate",
        "crate_index",
        "crate_meta",
        "owner",
        "crate_user",
        "crate_group",
        "crate_author",
        "crate_author_to_crate",
        "crate_keyword",
        "crate_keyword_to_crate",
        "crate_category",
        "crate_category_to_crate",
        "cratesio_crate",
        "cratesio_index",
        "cratesio_meta",
        "doc_queue",
        "webhook",
        "webhook_queue",
    ];

    for table in required_tables {
        if !manager.has_table(table).await? {
            return Err(DbErr::Custom(format!(
                "Missing table '{}'. Please ensure you have first upgraded to Kellnr v5.14.0 \
                before upgrading to v6.0.0.",
                table
            )));
        }
    }

    // Check for key columns that should exist
    let column_checks = [
        ("user", "is_read_only"),
        ("krate", "restricted_download"),
        ("krate", "e_tag"),
        ("krate", "original_name"),
        ("crate_index", "pubtime"),
        ("cratesio_index", "pubtime"),
        ("cratesio_crate", "max_version"),
        ("cratesio_meta", "documentation"),
    ];

    for (table, column) in column_checks {
        if !manager.has_column(table, column).await? {
            return Err(DbErr::Custom(format!(
                "Missing column '{}.{}'. Please ensure you have first upgraded to Kellnr v5.14.0 \
                before upgrading to v6.0.0.",
                table, column
            )));
        }
    }

    // Check for key indices
    let index_checks = [
        ("auth_token", "idx_auth_token_token"),
        ("owner", "idx-owner"),
        ("crate_user", "idx-crate-user"),
        ("crate_index", "idx-crate-index"),
        ("crate_meta", "idx-crate-meta"),
        ("group_user", "idx-group-user"),
        ("crate_group", "idx-crate-group"),
        ("cratesio_index", "idx-crates-io-index-fk"),
        ("cratesio_meta", "idx-crates-io-meta"),
        ("cratesio_crate", "idx-crates-io-name"),
    ];

    for (table, index) in index_checks {
        if !manager.has_index(table, index).await? {
            return Err(DbErr::Custom(format!(
                "Missing index '{}' on table '{}'. Please ensure you have first upgraded to \
                Kellnr v5.14.0 before upgrading to v6.0.0.",
                index, table
            )));
        }
    }

    Ok(())
}

/// Delete old migration entries from seaql_migrations
async fn cleanup_old_migrations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();

    for migration in OLD_MIGRATIONS {
        let sql = format!(
            "DELETE FROM seaql_migrations WHERE version = '{}'",
            migration
        );
        db.execute_unprepared(&sql).await?;
    }

    info!("Cleaned up {} old migration entries", OLD_MIGRATIONS.len());
    Ok(())
}
