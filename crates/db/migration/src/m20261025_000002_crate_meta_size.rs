use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_settings::cli::settings_from_process;
use kellnr_storage::init_storage;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;
use tracing::{info, warn};

use crate::iden::{CrateIndexIden, CrateMetaIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Size of the published .crate file in bytes, shown in the version list.
        // Zero means unknown, which is what versions keep whose size cannot be
        // read below.
        manager
            .alter_table(
                Table::alter()
                    .table(CrateMetaIden::Table)
                    .add_column(
                        ColumnDef::new(CrateMetaIden::Size)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        fill_sizes(manager.get_connection()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CrateMetaIden::Table)
                    .drop_column(CrateMetaIden::Size)
                    .to_owned(),
            )
            .await
    }
}

/// Reads the size of every published version from the crate storage. Only the
/// metadata of the files is read, not their content. The file is named after
/// the crate name the version was published with, which the index keeps. It
/// can differ from the crate's name in case and in "-" versus "_".
async fn fill_sizes(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    let versions = db
        .query_all(
            Query::select()
                .column((CrateMetaIden::Table, CrateMetaIden::Id))
                .column((CrateMetaIden::Table, CrateMetaIden::Version))
                .column((CrateIndexIden::Table, CrateIndexIden::Name))
                .from(CrateMetaIden::Table)
                .inner_join(
                    CrateIndexIden::Table,
                    Cond::all()
                        .add(
                            Expr::col((CrateMetaIden::Table, CrateMetaIden::CrateFk))
                                .equals((CrateIndexIden::Table, CrateIndexIden::CrateFk)),
                        )
                        .add(
                            Expr::col((CrateMetaIden::Table, CrateMetaIden::Version))
                                .equals((CrateIndexIden::Table, CrateIndexIden::Vers)),
                        ),
                ),
        )
        .await?;

    if versions.is_empty() {
        return Ok(());
    }
    let Some(storage) = crate_storage() else {
        warn!("Cannot open the crate storage, the size of the crates stays unknown");
        return Ok(());
    };

    info!("Reading the size of {} crate files", versions.len());
    for row in versions {
        let id: i64 = row.try_get("", "id")?;
        let version: String = row.try_get("", "version")?;
        let name: String = row.try_get("", "name")?;

        let size = match storage
            .size(
                &OriginalName::from_unchecked(name.clone()),
                &Version::from_unchecked_str(&version),
            )
            .await
        {
            Ok(size) => <i64 as TryFrom<u64>>::try_from(size).unwrap_or(i64::MAX),
            Err(e) => {
                warn!("Size of crate {name} {version} stays unknown: {e}");
                continue;
            }
        };

        db.execute(
            Query::update()
                .table(CrateMetaIden::Table)
                .value(CrateMetaIden::Size, size)
                .and_where(Expr::col(CrateMetaIden::Id).eq(id)),
        )
        .await?;
    }
    Ok(())
}

/// The crate storage of the running Kellnr. The migration runs before the
/// settings are passed around, so it resolves them itself.
fn crate_storage() -> Option<KellnrCrateStorage> {
    let settings = settings_from_process()?;
    let storage = init_storage(&settings.crates_path_or_bucket(), &settings)
        .inspect_err(|e| warn!("Failed to open the crate storage: {e}"))
        .ok()?;
    Some(KellnrCrateStorage::new(&settings, storage))
}
