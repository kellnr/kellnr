use crate::sea_orm::ActiveValue::Set;
use crate::sea_orm::{ActiveModelTrait, EntityTrait};
use sea_orm::{ModelTrait, Related};
use sea_orm_migration::prelude::*;
use settings::{get_settings, Settings};
use tracing::{debug, error};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("cratesio_crate", "max_version").await? {
            debug!("Column cratesio_crate.max_version does not exist. Creating...");
            manager
                .alter_table(
                    Table::alter()
                        .table(CratesIoIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CratesIoIden::MaxVersion)
                                .text()
                                .not_null()
                                .default("0.0.0"),
                        )
                        .to_owned(),
                )
                .await?;

            fill_max_version(&manager.get_connection()).await?;
        }

        if !manager.has_column("cratesio_meta", "documentation").await? {
            debug!("Column cratesio_meta.documentation does not exist. Creating...");
            manager
                .alter_table(
                    Table::alter()
                        .table(CratesIoMetaIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CratesIoMetaIden::Documentation).text(),
                        )
                        .to_owned(),
                )
                .await?;

            fill_documentation(&manager.get_connection()).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoIden::Table)
                    .drop_column(CratesIoIden::MaxVersion)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoMetaIden::Table)
                    .drop_column(CratesIoMetaIden::Documentation)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

async fn fill_max_version(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    use crate::m20220101_0000010_create_table_entities::{cratesio_crate, cratesio_index};

    let crates = cratesio_index::Entity::find().find_also_related(cratesio_crate::Entity).all(db).await?;

    for (ci, c) in crates {
        if c.is_none() {
            error!("No Crate found for index with id {}", ci.id);
            continue;
        }

        let version = ci.vers.clone();
        let mut m: cratesio_crate::ActiveModel = c.unwrap().into();
        m.max_version = Set(version);
        m.update(db).await?;
    }

    Ok(())
}

async fn fill_documentation(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    use crate::m20220101_0000010_create_table_entities::{cratesio_crate, cratesio_meta};

    let crates = cratesio_meta::Entity::find().find_also_related(cratesio_crate::Entity).all(db).await?;

    for (cm, c) in crates {
        if c.is_none() {
            error!("No Crate found for metadata with id {}", cm.id);
            continue;
        }

        let name = c.unwrap().name;
        let version = cm.version.clone();
        let mut m: cratesio_meta::ActiveModel = cm.into();
        m.documentation = Set(Some(format!(
            "https://docs.rs/{}/{}",
            name,
            version,
        )));
        m.update(db).await?;
    }
    Ok(())
}

#[derive(Iden)]
pub enum CratesIoIden {
    #[iden = "cratesio_crate"]
    Table,
    MaxVersion,
}

#[derive(Iden)]
pub enum CratesIoMetaIden {
    #[iden = "cratesio_meta"]
    Table,
    Documentation,
}
