use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Link tables between crates and their keywords, categories and authors, with
/// the name of the new unique index and the column that links to the tag.
const LINK_TABLES: [(&str, &str, &str); 3] = [
    (
        "crate_keyword_to_crate",
        "idx-crate-keyword-to-crate",
        "keyword_fk",
    ),
    (
        "crate_category_to_crate",
        "idx-crate-category-to-crate",
        "category_fk",
    ),
    (
        "crate_author_to_crate",
        "idx-crate-author-to-crate",
        "author_fk",
    ),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        remove_duplicate_links(manager.get_connection()).await?;

        for (table, index, tag_fk) in LINK_TABLES {
            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name(index)
                        .table(Alias::new(table))
                        .col(Alias::new("crate_fk"))
                        .col(Alias::new(tag_fk))
                        .unique()
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (table, index, _) in LINK_TABLES {
            manager
                .drop_index(
                    Index::drop()
                        .name(index)
                        .table(Alias::new(table))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

/// A manifest that lists a keyword, category or author twice linked it twice to
/// the crate. Removes the duplicates, so the unique indices can be created, and
/// keeps the first link.
pub async fn remove_duplicate_links<C: ConnectionTrait>(db: &C) -> Result<(), DbErr> {
    for (table, _, tag_fk) in LINK_TABLES {
        db.execute_unprepared(&format!(
            "DELETE FROM {table} WHERE id NOT IN \
             (SELECT MIN(id) FROM {table} GROUP BY crate_fk, {tag_fk})"
        ))
        .await?;
    }
    Ok(())
}
