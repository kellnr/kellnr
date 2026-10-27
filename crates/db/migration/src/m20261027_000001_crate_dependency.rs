use kellnr_common::crate_dependents::LocalDependency;
use kellnr_common::index_metadata::IndexDep;
use sea_orm::{ConnectionTrait, QueryResult};
use sea_orm_migration::prelude::*;
use tracing::{info, warn};

use crate::iden::{CrateDependencyIden, CrateIndexIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The dependencies of every version on crates of the same registry,
        // taken from the deps of the index. They answer which crates depend
        // on a crate without parsing the deps of all versions. Removed
        // together with their version.
        manager
            .create_table(
                Table::create()
                    .table(CrateDependencyIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateDependencyIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CrateDependencyIden::CrateIndexFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CrateDependencyIden::Name).text().not_null())
                    .col(
                        ColumnDef::new(CrateDependencyIden::VersionReq)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CrateDependencyIden::Kind).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_dependency_crate_index_fk")
                            .from(
                                CrateDependencyIden::Table,
                                CrateDependencyIden::CrateIndexFk,
                            )
                            .to(CrateIndexIden::Table, CrateIndexIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_crate_dependency_name")
                    .table(CrateDependencyIden::Table)
                    .col(CrateDependencyIden::Name)
                    .to_owned(),
            )
            .await?;

        // Deleting a version looks up its dependencies by the foreign key
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_crate_dependency_crate_index_fk")
                    .table(CrateDependencyIden::Table)
                    .col(CrateDependencyIden::CrateIndexFk)
                    .to_owned(),
            )
            .await?;

        fill_dependencies(manager.get_connection()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CrateDependencyIden::Table).to_owned())
            .await
    }
}

/// The dependencies of the version `id` on crates of the registry. Dependencies
/// that cannot be read are skipped with a warning instead of failing the
/// migration. They are only missing from the dependents of a crate.
fn local_dependencies(id: i64, row: &QueryResult) -> Vec<LocalDependency> {
    let deps = match row.try_get::<serde_json::Value>("", "deps") {
        Ok(serde_json::Value::Array(deps)) => deps,
        Ok(deps) => {
            warn!("Skipping the dependencies of crate index {id}, they are not a list: {deps}");
            return Vec::new();
        }
        Err(e) => {
            warn!("Skipping the dependencies of crate index {id}, they cannot be read: {e}");
            return Vec::new();
        }
    };
    deps.into_iter()
        .filter_map(|dep| {
            serde_json::from_value::<IndexDep>(dep.clone())
                .inspect_err(|e| {
                    warn!("Skipping invalid dependency {dep} of crate index {id}: {e}");
                })
                .ok()
        })
        .filter_map(|dep| LocalDependency::from_index_dep(&dep))
        .collect()
}

/// Number of versions read at once, to not load the whole index into memory
const PAGE_SIZE: u64 = 1000;

/// Stores the dependencies of all existing versions
pub async fn fill_dependencies<C: ConnectionTrait>(db: &C) -> Result<(), DbErr> {
    let mut last_id = 0;
    let mut count = 0;
    loop {
        let rows = db
            .query_all(
                Query::select()
                    .columns([CrateIndexIden::Id, CrateIndexIden::Deps])
                    .from(CrateIndexIden::Table)
                    .and_where(Expr::col(CrateIndexIden::Id).gt(last_id))
                    .and_where(Expr::col(CrateIndexIden::Deps).is_not_null())
                    .order_by(CrateIndexIden::Id, Order::Asc)
                    .limit(PAGE_SIZE),
            )
            .await?;
        let Some(last) = rows.last() else {
            break;
        };
        last_id = last.try_get::<i64>("", "id")?;

        let mut dependencies = Vec::new();
        for row in &rows {
            let id: i64 = row.try_get("", "id")?;
            dependencies.extend(local_dependencies(id, row).into_iter().map(|dep| (id, dep)));
        }
        count += dependencies.len();

        // Chunked to stay below the bind parameter limit of SQLite
        for chunk in dependencies.chunks(1000) {
            let mut insert = Query::insert()
                .into_table(CrateDependencyIden::Table)
                .columns([
                    CrateDependencyIden::CrateIndexFk,
                    CrateDependencyIden::Name,
                    CrateDependencyIden::VersionReq,
                    CrateDependencyIden::Kind,
                ])
                .to_owned();
            for (id, dep) in chunk {
                insert.values_panic([
                    (*id).into(),
                    dep.name.clone().into(),
                    dep.version_req.clone().into(),
                    dep.kind.clone().into(),
                ]);
            }
            db.execute(&insert).await?;
        }
    }
    info!("Stored {count} dependencies on crates of the registry");
    Ok(())
}
