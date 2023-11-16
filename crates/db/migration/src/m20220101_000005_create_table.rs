use crate::m20220101_000005_create_table_entities::{
    crate_author, crate_author_to_crate, crate_category, crate_category_to_crate, crate_index,
    crate_keyword, crate_keyword_to_crate, crate_meta, krate,
};
use crate::old_index_metadata::OldIndexDep;
use crate::old_index_metadata::OldIndexMetadata;
use common::index_metadata::metadata_path;
use common::version::Version;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::prelude::*;
use serde::{Deserialize, Serialize};
use settings::{get_settings, Settings};
use std::collections::HashMap;
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

        if manager.has_column("krate", "original_name").await? {
            debug!("Column krate.original_name already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateIden::OriginalName)
                                .text()
                                // .unique_key() Adding a unique key here would fail on Sqlite
                                .not_null()
                                .default("".to_string()),
                        )
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(CrateAuthorIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateAuthorIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateAuthorIden::Author)
                            .text()
                            .unique_key()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CrateKeywordIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateKeywordIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateKeywordIden::Keyword)
                            .text()
                            .unique_key()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CrateCategoryIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateCategoryIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateCategoryIden::Category)
                            .text()
                            .unique_key()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CrateAuthorToCrateIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateAuthorToCrateIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateAuthorToCrateIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(
                                CrateAuthorToCrateIden::Table,
                                CrateAuthorToCrateIden::CrateFk,
                            )
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .col(
                        ColumnDef::new(CrateAuthorToCrateIden::AuthorFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("author_fk")
                            .from(
                                CrateAuthorToCrateIden::Table,
                                CrateAuthorToCrateIden::AuthorFk,
                            )
                            .to(CrateAuthorIden::Table, CrateAuthorIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CrateCategoryToCrateIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateCategoryToCrateIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateCategoryToCrateIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(
                                CrateCategoryToCrateIden::Table,
                                CrateCategoryToCrateIden::CrateFk,
                            )
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .col(
                        ColumnDef::new(CrateCategoryToCrateIden::CategoryFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("category_fk")
                            .from(
                                CrateCategoryToCrateIden::Table,
                                CrateCategoryToCrateIden::CategoryFk,
                            )
                            .to(CrateCategoryIden::Table, CrateCategoryIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CrateKeywordToCrateIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateKeywordToCrateIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateKeywordToCrateIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(
                                CrateKeywordToCrateIden::Table,
                                CrateKeywordToCrateIden::CrateFk,
                            )
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .col(
                        ColumnDef::new(CrateKeywordToCrateIden::KeywordFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("keyword_fk")
                            .from(
                                CrateKeywordToCrateIden::Table,
                                CrateKeywordToCrateIden::KeywordFk,
                            )
                            .to(CrateKeywordIden::Table, CrateKeywordIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CrateIndexIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateIndexIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(CrateIndexIden::Name).text().not_null())
                    .col(ColumnDef::new(CrateIndexIden::Vers).text().not_null())
                    .col(ColumnDef::new(CrateIndexIden::Deps).json_binary())
                    .col(ColumnDef::new(CrateIndexIden::Cksum).text().not_null())
                    .col(ColumnDef::new(CrateIndexIden::Features).json_binary())
                    .col(
                        ColumnDef::new(CrateIndexIden::Yanked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(CrateIndexIden::Links).text())
                    .col(
                        ColumnDef::new(CrateIndexIden::V)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(CrateCategoryIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(CrateIndexIden::Table, CrateIndexIden::CrateFk)
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        fill_new_columns(manager.get_connection(), &settings).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order of creation
        manager
            .drop_table(Table::drop().table(CrateCategoryIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateKeywordIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateAuthorIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateIndexIden::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CrateCategoryToCrateIden::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CrateKeywordToCrateIden::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CrateAuthorToCrateIden::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CrateIden::Table)
                    .drop_column(CrateIden::OriginalName)
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

        let c = c.unwrap();
        let version = Version::try_from(&cm.version).map_err(|e| DbErr::Custom(e.to_string()))?;
        let index_file = metadata_path(&crate_path, &c.name);
        let index = OldIndexMetadata::from_version(&index_file, &version)
            .map_err(|e| DbErr::Custom(e.to_string()))?;

        let crate_id = c.id;
        fill_authors(db, &index.authors, crate_id).await?;
        fill_categories(db, &index.categories, crate_id).await?;
        fill_keywords(db, &index.keywords, crate_id).await?;
        fill_crate_index(db, &index, crate_id).await?;

        // Safe the original name and then change the crate name to lower case
        let lower_case_name = c.name.to_lowercase();
        let original_name = c.name.clone();
        let mut c: krate::ActiveModel = c.into();
        c.name = Set(lower_case_name);
        c.original_name = Set(original_name);
        c.update(db).await?;
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Deps(Vec<OldIndexDep>);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Features(HashMap<String, Vec<String>>);

async fn fill_crate_index(
    db: &SchemaManagerConnection<'_>,
    index: &OldIndexMetadata,
    crate_id: i64,
) -> Result<(), DbErr> {
    let ci = crate_index::Entity::find()
        .filter(
            Cond::all()
                .add(crate_index::Column::Vers.eq(index.vers.to_string()))
                .add(crate_index::Column::CrateFk.eq(crate_id)),
        )
        .one(db)
        .await?;

    if ci.is_none() {
        let deps = if !index.deps.is_empty() {
            match serde_json::value::to_value(index.deps.clone()) {
                Ok(v) => Some(v),
                Err(e) => {
                    error!("Failed to serialize deps: {}", e);
                    return Err(DbErr::Custom(e.to_string()));
                }
            }
        } else {
            None
        };

        let features = if index.features.is_some() {
            match serde_json::value::to_value(index.features.clone()) {
                Ok(v) => Some(v),
                Err(e) => {
                    error!("Failed to serialize features: {}", e);
                    return Err(DbErr::Custom(e.to_string()));
                }
            }
        } else {
            None
        };

        let ci = crate_index::ActiveModel {
            vers: Set(index.vers.to_string()),
            name: Set(index.name.to_string()),
            deps: Set(deps),
            cksum: Set(index.cksum.to_string()),
            features: Set(features),
            yanked: Set(index.yanked),
            links: Set(index.links.clone()),
            crate_fk: Set(crate_id),
            ..Default::default()
        };

        ci.insert(db).await?;
    }

    Ok(())
}

async fn fill_authors(
    db: &SchemaManagerConnection<'_>,
    authors: &Option<Vec<String>>,
    crate_id: i64,
) -> Result<(), DbErr> {
    async fn insert_author(db: &SchemaManagerConnection<'_>, author: &str) -> Result<i64, DbErr> {
        let ca = crate_author::Entity::find()
            .filter(crate_author::Column::Author.eq(author))
            .one(db)
            .await?;

        let id = match ca {
            Some(ca) => ca.id,
            None => {
                let crate_author: crate_author::ActiveModel = crate_author::ActiveModel {
                    author: Set(author.to_string()),
                    ..Default::default()
                };

                crate_author.insert(db).await?.id
            }
        };

        Ok(id)
    }

    async fn insert_author_to_crate(
        db: &SchemaManagerConnection<'_>,
        author_id: i64,
        crate_id: i64,
    ) -> Result<(), DbErr> {
        let ca = crate_author_to_crate::Entity::find()
            .filter(crate_author_to_crate::Column::AuthorFk.eq(author_id))
            .filter(crate_author_to_crate::Column::CrateFk.eq(crate_id))
            .one(db)
            .await?;

        if ca.is_none() {
            let crate_author_to_crate: crate_author_to_crate::ActiveModel =
                crate_author_to_crate::ActiveModel {
                    author_fk: Set(author_id),
                    crate_fk: Set(crate_id),
                    ..Default::default()
                };

            crate_author_to_crate.insert(db).await?;
        }

        Ok(())
    }

    if authors.is_none() {
        return Ok(());
    }

    for author in authors.as_ref().unwrap() {
        let id = insert_author(db, author).await?;
        insert_author_to_crate(db, id, crate_id).await?;
    }

    Ok(())
}

async fn fill_categories(
    db: &SchemaManagerConnection<'_>,
    categories: &Option<Vec<String>>,
    crate_id: i64,
) -> Result<(), DbErr> {
    async fn insert_category(
        db: &SchemaManagerConnection<'_>,
        category: &str,
    ) -> Result<i64, DbErr> {
        let ca = crate_category::Entity::find()
            .filter(crate_category::Column::Category.eq(category))
            .one(db)
            .await?;

        let id = match ca {
            Some(ca) => ca.id,
            None => {
                let crate_category: crate_category::ActiveModel = crate_category::ActiveModel {
                    category: Set(category.to_string()),
                    ..Default::default()
                };

                crate_category.insert(db).await?.id
            }
        };

        Ok(id)
    }

    async fn category_to_crate(
        db: &SchemaManagerConnection<'_>,
        category_id: i64,
        crate_id: i64,
    ) -> Result<(), DbErr> {
        let ca = crate_category_to_crate::Entity::find()
            .filter(crate_category_to_crate::Column::CategoryFk.eq(category_id))
            .filter(crate_category_to_crate::Column::CrateFk.eq(crate_id))
            .one(db)
            .await?;

        if ca.is_none() {
            let crate_category_to_crate: crate_category_to_crate::ActiveModel =
                crate_category_to_crate::ActiveModel {
                    category_fk: Set(category_id),
                    crate_fk: Set(crate_id),
                    ..Default::default()
                };

            crate_category_to_crate.insert(db).await?;
        }

        Ok(())
    }

    if categories.is_none() {
        return Ok(());
    }

    for cats in categories.as_ref().unwrap() {
        let id = insert_category(db, cats).await?;
        category_to_crate(db, id, crate_id).await?;
    }

    Ok(())
}

async fn fill_keywords(
    db: &SchemaManagerConnection<'_>,
    keywords: &Option<Vec<String>>,
    crate_id: i64,
) -> Result<(), DbErr> {
    async fn insert_keywords(
        db: &SchemaManagerConnection<'_>,
        keyword: &str,
    ) -> Result<i64, DbErr> {
        let ca = crate_keyword::Entity::find()
            .filter(crate_keyword::Column::Keyword.eq(keyword))
            .one(db)
            .await?;

        let id = match ca {
            Some(ca) => ca.id,
            None => {
                let crate_keyword: crate_keyword::ActiveModel = crate_keyword::ActiveModel {
                    keyword: Set(keyword.to_string()),
                    ..Default::default()
                };

                crate_keyword.insert(db).await?.id
            }
        };

        Ok(id)
    }

    async fn keyword_to_crate(
        db: &SchemaManagerConnection<'_>,
        keyword_id: i64,
        crate_id: i64,
    ) -> Result<(), DbErr> {
        let ca = crate_keyword_to_crate::Entity::find()
            .filter(crate_keyword_to_crate::Column::KeywordFk.eq(keyword_id))
            .filter(crate_keyword_to_crate::Column::CrateFk.eq(crate_id))
            .one(db)
            .await?;

        if ca.is_none() {
            let crate_keyword_to_crate: crate_keyword_to_crate::ActiveModel =
                crate_keyword_to_crate::ActiveModel {
                    keyword_fk: Set(keyword_id),
                    crate_fk: Set(crate_id),
                    ..Default::default()
                };

            crate_keyword_to_crate.insert(db).await?;
        }

        Ok(())
    }

    if keywords.is_none() {
        return Ok(());
    }

    for kw in keywords.as_ref().unwrap() {
        let id = insert_keywords(db, kw).await?;
        keyword_to_crate(db, id, crate_id).await?;
    }

    Ok(())
}

#[derive(Iden)]
pub enum CrateIden {
    #[iden = "krate"]
    Table,
    Id,
    OriginalName,
}

#[derive(Iden)]
pub enum CrateAuthorIden {
    #[iden = "crate_author"]
    Table,
    Id,
    Author,
}

#[derive(Iden)]
pub enum CrateAuthorToCrateIden {
    #[iden = "crate_author_to_crate"]
    Table,
    Id,
    AuthorFk,
    CrateFk,
}

#[derive(Iden)]
pub enum CrateKeywordIden {
    #[iden = "crate_keyword"]
    Table,
    Id,
    Keyword,
}

#[derive(Iden)]
pub enum CrateKeywordToCrateIden {
    #[iden = "crate_keyword_to_crate"]
    Table,
    Id,
    KeywordFk,
    CrateFk,
}

#[derive(Iden)]
pub enum CrateCategoryIden {
    #[iden = "crate_category"]
    Table,
    Id,
    Category,
    CrateFk,
}

#[derive(Iden)]
pub enum CrateCategoryToCrateIden {
    #[iden = "crate_category_to_crate"]
    Table,
    Id,
    CategoryFk,
    CrateFk,
}

#[derive(Iden)]
pub enum CrateIndexIden {
    #[iden = "crate_index"]
    Table,
    Id,
    Name,
    Vers,
    Deps,
    Cksum,
    Features,
    Yanked,
    Links,
    V,
    CrateFk,
}
