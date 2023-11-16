use crate::old_index_metadata::OldIndexMetadata;
use crate::sea_orm::ActiveValue::Set;
use crate::sea_orm::{ActiveModelTrait, EntityTrait};
use common::index_metadata::metadata_path;
use common::version::Version;
use sea_orm_migration::prelude::*;
use settings::{get_settings, Settings};
use tracing::{debug, error};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let settings = get_settings().map_err(|e| DbErr::Custom(e.to_string()))?;

        // Manual check if the column exists is needed, as Sqlite does not support
        // ALTER TABLE IF COLUMN EXISTS. Without the check, the migration would fail
        // on Sqlite with an "duplicate column" error.
        if manager.has_column("crate_meta", "readme").await? {
            debug!("Column crate_meta.readme already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateMetaIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateMetaIden::Readme).text().null(),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new column readme");
        }

        if manager.has_column("crate_meta", "license").await? {
            debug!("Column crate_meta.license already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateMetaIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateMetaIden::License).text().null(),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new column license");
        }

        if manager.has_column("crate_meta", "license_file").await? {
            debug!("Column crate_meta.license_file already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateMetaIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateMetaIden::LicenseFile).text().null(),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new column license_file");
        }

        if manager.has_column("crate_meta", "documentation").await? {
            debug!("Column crate_meta.documentation already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateMetaIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateMetaIden::Documentation).text().null(),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new column documentation");
        }

        if manager.has_column("krate", "description").await? {
            debug!("Column krate.description already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateIden::Description).text().null(),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new column description");
        }

        if manager.has_column("krate", "homepage").await? {
            debug!("Column krate.homepage already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIden::Table)
                        .add_column_if_not_exists(ColumnDef::new(CrateIden::Homepage).text().null())
                        .to_owned(),
                )
                .await?;
            debug!("Added new column homepage");
        }

        if manager.has_column("krate", "repository").await? {
            debug!("Column krate.repository already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateIden::Repository).text().null(),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new repository homepage");
        }

        fill_new_columns(manager.get_connection(), &settings).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CrateMetaIden::Table)
                    .drop_column(CrateMetaIden::Readme)
                    .drop_column(CrateMetaIden::License)
                    .drop_column(CrateMetaIden::LicenseFile)
                    .drop_column(CrateMetaIden::Documentation)
                    .drop_column(CrateIden::Description)
                    .drop_column(CrateIden::Homepage)
                    .drop_column(CrateIden::Repository)
                    .to_owned(),
            )
            .await
    }
}

pub fn index_path(settings: &Settings) -> std::path::PathBuf {
    std::path::PathBuf::from(&settings.registry.data_dir)
        .join("git")
        .join("index")
}

async fn fill_new_columns(
    db: &SchemaManagerConnection<'_>,
    settings: &Settings,
) -> Result<(), DbErr> {
    debug!("Filling new columns");
    use crate::m20220101_000004_create_table_entities::{crate_meta, krate};

    let crate_path = index_path(settings);
    let crates = crate_meta::Entity::find()
        .find_also_related(krate::Entity)
        .all(db)
        .await?;

    for (cm, c) in crates {
        if c.is_none() {
            error!("No Crate found for metadata with id {}", cm.id);
            continue;
        }

        let version = Version::try_from(&cm.version).map_err(|e| DbErr::Custom(e.to_string()))?;
        let index_file = metadata_path(&crate_path, &c.as_ref().unwrap().name);
        let index = OldIndexMetadata::from_version(&index_file, &version)
            .map_err(|e| DbErr::Custom(e.to_string()))?;

        let mut crate_meta: crate_meta::ActiveModel = crate_meta::Entity::find_by_id(cm.id)
            .one(db)
            .await?
            .ok_or(DbErr::Custom(format!(
                "Crate metadata not found for id {}",
                cm.id
            )))?
            .into();

        let mut krate: krate::ActiveModel = krate::Entity::find_by_id(c.as_ref().unwrap().id)
            .one(db)
            .await?
            .ok_or(DbErr::Custom(format!(
                "Crate not found for id {}",
                c.as_ref().unwrap().id
            )))?
            .into();

        let doc_url = match index.documentation {
            Some(url) => Some(url),
            None => get_doc_url(&c.as_ref().unwrap().name, &version, settings),
        };

        krate.description = Set(index.description);
        krate.homepage = Set(index.homepage);
        krate.repository = Set(index.repository);
        crate_meta.readme = Set(index.readme);
        crate_meta.license = Set(index.license);
        crate_meta.license_file = Set(index.license_file);
        crate_meta.documentation = Set(doc_url);

        crate_meta.update(db).await?;
        krate.update(db).await?;
    }

    debug!("Filled new columns");
    Ok(())
}

fn get_doc_url(crate_name: &str, crate_version: &Version, settings: &Settings) -> Option<String> {
    let docs_name = crate_name_to_docs_name(crate_name);

    if doc_exists(crate_name, crate_version, settings) {
        Some(format!(
            "/docs/{}/{}/doc/{}/index.html",
            crate_name, crate_version, docs_name
        ))
    } else {
        None
    }
}

fn crate_name_to_docs_name(crate_name: &str) -> String {
    // Cargo replaces the `-` with `_` in the crate name when
    // docs are generated. As such, the docs folder name is not "foo-bar" but "foo_bar".
    crate_name.replace('-', "_")
}

fn doc_exists(crate_name: &str, crate_version: &str, settings: &Settings) -> bool {
    let docs_name = crate_name_to_docs_name(crate_name);
    settings
        .docs_path()
        .join(crate_name)
        .join(crate_version)
        .join("doc")
        .join(docs_name)
        .join("index.html")
        .exists()
}

#[derive(Iden)]
pub enum CrateMetaIden {
    #[iden = "crate_meta"]
    Table,
    Readme,
    License,
    LicenseFile,
    Documentation,
}

#[derive(Iden)]
pub enum CrateIden {
    #[iden = "krate"]
    Table,
    Description,
    Homepage,
    Repository,
}
