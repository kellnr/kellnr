use crate::m20220101_000006_create_table_entities::{crate_index, krate};
use common::index_metadata::IndexMetadata;
use hex::ToHex;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::prelude::*;
use sha2::{Digest, Sha256};
use tracing::debug;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Manual check if the column exists is needed, as Sqlite does not support
        // ALTER TABLE IF COLUMN EXISTS. Without the check, the migration would fail
        // on Sqlite with an "duplicate column" error.

        if manager.has_column("krate", "etag").await? {
            debug!("Column krate.etag already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateIden::ETag)
                                .string_len(64)
                                .not_null()
                                .default(""),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new column etag");
        }

        fill_new_column(manager.get_connection()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CrateIden::Table)
                    .drop_column(CrateIden::ETag)
                    .to_owned(),
            )
            .await
    }
}

async fn fill_new_column(conn: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    let crates = krate::Entity::find().all(conn).await?;

    for c in crates {
        let etag = compute_etag(conn, &c.name, c.id).await?;
        let mut krate: krate::ActiveModel = c.into();
        krate.e_tag = Set(etag);
        krate.update(conn).await?;
    }

    Ok(())
}

async fn compute_etag(
    db_con: &SchemaManagerConnection<'_>,
    crate_name: &str,
    crate_id: i64,
) -> Result<String, DbErr> {
    let crate_indices = crate_index::Entity::find()
        .filter(crate_index::Column::CrateFk.eq(crate_id))
        .all(db_con)
        .await?;

    let mut index_metadata = vec![];
    for ci in crate_indices {
        let deps = match ci.deps {
            Some(ref deps) => serde_json::value::from_value(deps.to_owned()).map_err(|e| {
                DbErr::Custom(format!(
                    "Failed to deserialize crate dependencies of {crate_name}: {e}"
                ))
            })?,
            None => vec![],
        };
        let features = ci.features.clone().unwrap_or_default();
        let features = serde_json::value::from_value(features).map_err(|e| {
            DbErr::Custom(format!(
                "Failed to deserialize crate features of {crate_name}: {e}"
            ))
        })?;

        let cm = IndexMetadata {
            name: crate_name.to_string(),
            vers: ci.vers.to_string(),
            deps,
            cksum: ci.cksum.to_string(),
            features,
            features2: None,
            yanked: ci.yanked,
            links: ci.links.clone(),
            v: Some(ci.v as u32),
        };
        index_metadata.push(cm);
    }

    let data = IndexMetadata::serialize_indices(&index_metadata)
        .map(|idx| idx.into_bytes())
        .map_err(|e| {
            DbErr::Custom(format!(
                "Failed to serialize crate indices of {crate_name}: {e}"
            ))
        })?;

    Ok(Sha256::digest(data).encode_hex())
}

#[derive(Iden)]
pub enum CrateIden {
    #[iden = "krate"]
    Table,
    ETag,
}
