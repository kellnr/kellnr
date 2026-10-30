use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Tables that hold one row per crate, identified by the `name` column.
const CRATE_TABLES: [&str; 2] = ["krate", "cratesio_crate"];

/// SQL expression of the canonical form of a crate name: lowercase with `-`
/// replaced by `_`, like cargo and crates.io compare crate names.
fn canonical(column: &str) -> String {
    format!("REPLACE(LOWER({column}), '-', '_')")
}

/// Stores the crate names in their canonical form, so that names that only
/// differ in case or in `-` versus `_` are the same crate everywhere. The
/// existing unique keys on the name columns then prevent such duplicates.
///
/// Older versions of Kellnr allowed `foo-bar` and `foo_bar` as two crates. The
/// migration fails without changing anything if the database holds such
/// crates, as only an admin can decide which one to keep.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        canonicalize_crate_names(manager.get_connection()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        // The dependencies first, they are matched by the canonical name.
        db.execute_unprepared(
            "UPDATE crate_dependency SET name = COALESCE(\
             (SELECT LOWER(krate.original_name) FROM krate WHERE krate.name = crate_dependency.name), \
             name)",
        )
        .await?;
        for table in CRATE_TABLES {
            db.execute_unprepared(&format!("UPDATE {table} SET name = LOWER(original_name)"))
                .await?;
        }
        Ok(())
    }
}

/// Converts the crate names to their canonical form, or fails if two crates
/// have the same canonical name. Public for the database tests.
pub async fn canonicalize_crate_names<C: ConnectionTrait>(db: &C) -> Result<(), DbErr> {
    let mut conflicts = Vec::new();
    for table in CRATE_TABLES {
        conflicts.extend(canonical_conflicts(db, table).await?);
    }
    if !conflicts.is_empty() {
        return Err(DbErr::Migration(format!(
            "Cannot upgrade the database: cargo treats crate names that only differ in case or in \
             '-' versus '_' as the same crate, but the database contains several crates with such \
             names: {}. Delete or rename all but one crate of each group, then start Kellnr again. \
             No data was changed.",
            conflicts.join("; ")
        )));
    }

    for table in CRATE_TABLES {
        db.execute_unprepared(&format!("UPDATE {table} SET name = {}", canonical("name")))
            .await?;
    }
    db.execute_unprepared(&format!(
        "UPDATE crate_dependency SET name = {}",
        canonical("name")
    ))
    .await?;
    Ok(())
}

/// The groups of crates in `table` that have the same canonical name, as text
/// for the error message, like `foo-bar and foo_bar (crates.io cache)`.
async fn canonical_conflicts<C: ConnectionTrait>(
    db: &C,
    table: &str,
) -> Result<Vec<String>, DbErr> {
    let name = canonical("name");
    let rows = db
        .query_all_raw(Statement::from_string(
            db.get_database_backend(),
            format!(
                "SELECT {name} AS canonical_name, original_name FROM {table} \
                 WHERE {name} IN (SELECT {name} FROM {table} GROUP BY {name} HAVING COUNT(*) > 1) \
                 ORDER BY canonical_name, original_name"
            ),
        ))
        .await?;

    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows {
        let canonical_name: String = row.try_get("", "canonical_name")?;
        let original_name: String = row.try_get("", "original_name")?;
        groups
            .entry(canonical_name)
            .or_default()
            .push(original_name);
    }

    let location = if table == "cratesio_crate" {
        " (crates.io cache)"
    } else {
        ""
    };
    Ok(groups
        .into_values()
        .map(|names| format!("{}{location}", names.join(" and ")))
        .collect())
}
