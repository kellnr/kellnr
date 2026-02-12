//! Baseline migration for Kellnr v6.0.0
//!
//! This migration creates the complete database schema for fresh installations.
//! For upgrades from v5.14.0, the schema already exists and this migration is
//! skipped by the upgrade migration.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ========================================
        // User Management Tables
        // ========================================

        // user table
        manager
            .create_table(
                Table::create()
                    .table(UserIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserIden::Name)
                            .text()
                            .unique_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserIden::Pwd).text().not_null())
                    .col(ColumnDef::new(UserIden::Salt).text().not_null())
                    .col(ColumnDef::new(UserIden::IsAdmin).boolean().not_null())
                    .col(
                        ColumnDef::new(UserIden::IsReadOnly)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // auth_token table
        manager
            .create_table(
                Table::create()
                    .table(AuthTokenIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuthTokenIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(AuthTokenIden::Name).text().not_null())
                    .col(ColumnDef::new(AuthTokenIden::Token).text().not_null())
                    .col(
                        ColumnDef::new(AuthTokenIden::UserFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(AuthTokenIden::Table, AuthTokenIden::UserFk)
                            .to(UserIden::Table, UserIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique index on auth_token.token
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_auth_token_token")
                    .table(AuthTokenIden::Table)
                    .col(AuthTokenIden::Token)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // session table
        manager
            .create_table(
                Table::create()
                    .table(SessionIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SessionIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(SessionIden::Token)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(SessionIden::Created).text().not_null())
                    .col(ColumnDef::new(SessionIden::UserFk).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(SessionIden::Table, SessionIden::UserFk)
                            .to(UserIden::Table, UserIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // ========================================
        // Group Management Tables
        // ========================================

        // group table
        manager
            .create_table(
                Table::create()
                    .table(GroupIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GroupIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(GroupIden::Name)
                            .text()
                            .unique_key()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // group_user table
        manager
            .create_table(
                Table::create()
                    .table(GroupUserIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GroupUserIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(GroupUserIden::GroupFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("group_fk")
                            .from(GroupUserIden::Table, GroupUserIden::GroupFk)
                            .to(GroupIden::Table, GroupIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(GroupUserIden::UserFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(GroupUserIden::Table, GroupUserIden::UserFk)
                            .to(UserIden::Table, UserIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // ========================================
        // Crate Management Tables
        // ========================================

        // krate table
        manager
            .create_table(
                Table::create()
                    .table(CrateIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateIden::Name)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(CrateIden::MaxVersion)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(CrateIden::TotalDownloads)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CrateIden::LastUpdated)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(ColumnDef::new(CrateIden::Description).text())
                    .col(ColumnDef::new(CrateIden::Homepage).text())
                    .col(ColumnDef::new(CrateIden::Repository).text())
                    .col(
                        ColumnDef::new(CrateIden::OriginalName)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(CrateIden::ETag)
                            .string_len(64)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(CrateIden::RestrictedDownload)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // crate_index table
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
                        ColumnDef::new(CrateIndexIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CrateIndexIden::Pubtime).date_time())
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

        // crate_meta table
        manager
            .create_table(
                Table::create()
                    .table(CrateMetaIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateMetaIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(CrateMetaIden::Version).text().not_null())
                    .col(ColumnDef::new(CrateMetaIden::Created).text().not_null())
                    .col(
                        ColumnDef::new(CrateMetaIden::Downloads)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CrateMetaIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CrateMetaIden::Readme).text())
                    .col(ColumnDef::new(CrateMetaIden::License).text())
                    .col(ColumnDef::new(CrateMetaIden::LicenseFile).text())
                    .col(ColumnDef::new(CrateMetaIden::Documentation).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(CrateMetaIden::Table, CrateMetaIden::CrateFk)
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // ========================================
        // Crate Association Tables
        // ========================================

        // owner table
        manager
            .create_table(
                Table::create()
                    .table(OwnerIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OwnerIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(OwnerIden::CrateFk).big_integer().not_null())
                    .col(ColumnDef::new(OwnerIden::UserFk).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(OwnerIden::Table, OwnerIden::CrateFk)
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(OwnerIden::Table, OwnerIden::UserFk)
                            .to(UserIden::Table, UserIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // crate_user table
        manager
            .create_table(
                Table::create()
                    .table(CrateUserIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateUserIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateUserIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CrateUserIden::UserFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(CrateUserIden::Table, CrateUserIden::CrateFk)
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(CrateUserIden::Table, CrateUserIden::UserFk)
                            .to(UserIden::Table, UserIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // crate_group table
        manager
            .create_table(
                Table::create()
                    .table(CrateGroupIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateGroupIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateGroupIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(CrateGroupIden::Table, CrateGroupIden::CrateFk)
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(CrateGroupIden::GroupFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("group_fk")
                            .from(CrateGroupIden::Table, CrateGroupIden::GroupFk)
                            .to(GroupIden::Table, GroupIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // ========================================
        // Crate Metadata Tables
        // ========================================

        // crate_author table
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

        // crate_author_to_crate table
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

        // crate_keyword table
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

        // crate_keyword_to_crate table
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

        // crate_category table
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

        // crate_category_to_crate table
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

        // ========================================
        // Crates.io Proxy Tables
        // ========================================

        // cratesio_crate table
        manager
            .create_table(
                Table::create()
                    .table(CratesIoIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CratesIoIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CratesIoIden::Name)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(CratesIoIden::OriginalName)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(CratesIoIden::ETag).string_len(64).not_null())
                    .col(ColumnDef::new(CratesIoIden::LastModified).text().not_null())
                    .col(ColumnDef::new(CratesIoIden::Description).text())
                    .col(
                        ColumnDef::new(CratesIoIden::TotalDownloads)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CratesIoIden::MaxVersion)
                            .text()
                            .not_null()
                            .default("0.0.0"),
                    )
                    .to_owned(),
            )
            .await?;

        // cratesio_index table
        manager
            .create_table(
                Table::create()
                    .table(CratesIoIndexIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CratesIoIndexIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(CratesIoIndexIden::Name).text().not_null())
                    .col(ColumnDef::new(CratesIoIndexIden::Vers).text().not_null())
                    .col(ColumnDef::new(CratesIoIndexIden::Deps).json_binary())
                    .col(ColumnDef::new(CratesIoIndexIden::Cksum).text().not_null())
                    .col(ColumnDef::new(CratesIoIndexIden::Features).json_binary())
                    .col(ColumnDef::new(CratesIoIndexIden::Features2).json_binary())
                    .col(
                        ColumnDef::new(CratesIoIndexIden::Yanked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(CratesIoIndexIden::Links).text())
                    .col(
                        ColumnDef::new(CratesIoIndexIden::V)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(CratesIoIndexIden::CratesIoFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CratesIoIndexIden::Pubtime).date_time())
                    .foreign_key(
                        ForeignKey::create()
                            .name("cratesio_fk")
                            .from(CratesIoIndexIden::Table, CratesIoIndexIden::CratesIoFk)
                            .to(CratesIoIden::Table, CratesIoIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // cratesio_meta table
        manager
            .create_table(
                Table::create()
                    .table(CratesIoMetaIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CratesIoMetaIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(CratesIoMetaIden::Version).text().not_null())
                    .col(
                        ColumnDef::new(CratesIoMetaIden::Downloads)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CratesIoMetaIden::CratesIoFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CratesIoMetaIden::Documentation).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("cratesio_fk")
                            .from(CratesIoMetaIden::Table, CratesIoMetaIden::CratesIoFk)
                            .to(CratesIoIden::Table, CratesIoIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // ========================================
        // Queue Tables
        // ========================================

        // doc_queue table
        manager
            .create_table(
                Table::create()
                    .table(DocQueueIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DocQueueIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(DocQueueIden::Krate).text().not_null())
                    .col(ColumnDef::new(DocQueueIden::Version).text().not_null())
                    .col(ColumnDef::new(DocQueueIden::Path).text().not_null())
                    .to_owned(),
            )
            .await?;

        // webhook table
        manager
            .create_table(
                Table::create()
                    .table(WebhookIden::Table)
                    .if_not_exists()
                    .col(pk_uuid(WebhookIden::Id))
                    .col(string(WebhookIden::Event))
                    .col(string(WebhookIden::CallbackUrl))
                    .col(string_null(WebhookIden::Name))
                    .to_owned(),
            )
            .await?;

        // webhook_queue table
        manager
            .create_table(
                Table::create()
                    .table(WebhookQueueIden::Table)
                    .if_not_exists()
                    .col(pk_uuid(WebhookQueueIden::Id))
                    .col(uuid(WebhookQueueIden::WebhookFk))
                    .col(json(WebhookQueueIden::Payload))
                    .col(timestamp_with_time_zone_null(WebhookQueueIden::LastAttempt))
                    .col(timestamp_with_time_zone(WebhookQueueIden::NextAttempt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("webhook_fk")
                            .from(WebhookQueueIden::Table, WebhookQueueIden::WebhookFk)
                            .to(WebhookIden::Table, WebhookIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // ========================================
        // Unique Indices
        // ========================================

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-owner")
                    .table(OwnerIden::Table)
                    .col(OwnerIden::CrateFk)
                    .col(OwnerIden::UserFk)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-crate-user")
                    .table(CrateUserIden::Table)
                    .col(CrateUserIden::CrateFk)
                    .col(CrateUserIden::UserFk)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-crate-index")
                    .table(CrateIndexIden::Table)
                    .col(CrateIndexIden::CrateFk)
                    .col(CrateIndexIden::Vers)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-crate-meta")
                    .table(CrateMetaIden::Table)
                    .col(CrateMetaIden::CrateFk)
                    .col(CrateMetaIden::Version)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-group-user")
                    .table(GroupUserIden::Table)
                    .col(GroupUserIden::UserFk)
                    .col(GroupUserIden::GroupFk)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-crate-group")
                    .table(CrateGroupIden::Table)
                    .col(CrateGroupIden::CrateFk)
                    .col(CrateGroupIden::GroupFk)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // ========================================
        // Crates.io Indices
        // ========================================

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-crates-io-index-fk")
                    .table(CratesIoIndexIden::Table)
                    .col(CratesIoIndexIden::CratesIoFk)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-crates-io-meta")
                    .table(CratesIoMetaIden::Table)
                    .col(CratesIoMetaIden::CratesIoFk)
                    .col(CratesIoMetaIden::Version)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-crates-io-name")
                    .table(CratesIoIden::Table)
                    .col(CratesIoIden::Name)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indices first
        manager
            .drop_index(Index::drop().name("idx-crates-io-name").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-crates-io-meta").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-crates-io-index-fk").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-crate-group").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-group-user").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-crate-meta").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-crate-index").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-crate-user").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-owner").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_auth_token_token").to_owned())
            .await?;

        // Drop tables in reverse order
        manager
            .drop_table(Table::drop().table(WebhookQueueIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WebhookIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(DocQueueIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CratesIoMetaIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CratesIoIndexIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CratesIoIden::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CrateCategoryToCrateIden::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(CrateCategoryIden::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CrateKeywordToCrateIden::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(CrateKeywordIden::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CrateAuthorToCrateIden::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(CrateAuthorIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateGroupIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateUserIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OwnerIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateMetaIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateIndexIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GroupUserIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GroupIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SessionIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AuthTokenIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserIden::Table).to_owned())
            .await
    }
}

// ========================================
// Table Identifiers
// ========================================

#[derive(Iden)]
enum UserIden {
    #[iden = "user"]
    Table,
    Id,
    Name,
    Pwd,
    Salt,
    IsAdmin,
    IsReadOnly,
}

#[derive(Iden)]
enum AuthTokenIden {
    #[iden = "auth_token"]
    Table,
    Id,
    Name,
    Token,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
enum SessionIden {
    #[iden = "session"]
    Table,
    Id,
    Token,
    Created,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
enum GroupIden {
    #[iden = "group"]
    Table,
    Id,
    Name,
}

#[derive(Iden)]
enum GroupUserIden {
    #[iden = "group_user"]
    Table,
    Id,
    #[iden = "group_fk"]
    GroupFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
enum CrateIden {
    #[iden = "krate"]
    Table,
    Id,
    Name,
    MaxVersion,
    TotalDownloads,
    LastUpdated,
    Description,
    Homepage,
    Repository,
    OriginalName,
    ETag,
    RestrictedDownload,
}

#[derive(Iden)]
enum CrateIndexIden {
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
    #[iden = "crate_fk"]
    CrateFk,
    Pubtime,
}

#[derive(Iden)]
enum CrateMetaIden {
    #[iden = "crate_meta"]
    Table,
    Id,
    Version,
    Created,
    Downloads,
    #[iden = "crate_fk"]
    CrateFk,
    Readme,
    License,
    LicenseFile,
    Documentation,
}

#[derive(Iden)]
enum OwnerIden {
    #[iden = "owner"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
enum CrateUserIden {
    #[iden = "crate_user"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
enum CrateGroupIden {
    #[iden = "crate_group"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "group_fk"]
    GroupFk,
}

#[derive(Iden)]
enum CrateAuthorIden {
    #[iden = "crate_author"]
    Table,
    Id,
    Author,
}

#[derive(Iden)]
enum CrateAuthorToCrateIden {
    #[iden = "crate_author_to_crate"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "author_fk"]
    AuthorFk,
}

#[derive(Iden)]
enum CrateKeywordIden {
    #[iden = "crate_keyword"]
    Table,
    Id,
    Keyword,
}

#[derive(Iden)]
enum CrateKeywordToCrateIden {
    #[iden = "crate_keyword_to_crate"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "keyword_fk"]
    KeywordFk,
}

#[derive(Iden)]
enum CrateCategoryIden {
    #[iden = "crate_category"]
    Table,
    Id,
    Category,
}

#[derive(Iden)]
enum CrateCategoryToCrateIden {
    #[iden = "crate_category_to_crate"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "category_fk"]
    CategoryFk,
}

#[derive(Iden)]
enum CratesIoIden {
    #[iden = "cratesio_crate"]
    Table,
    Id,
    Name,
    OriginalName,
    ETag,
    LastModified,
    Description,
    TotalDownloads,
    MaxVersion,
}

#[derive(Iden)]
enum CratesIoIndexIden {
    #[iden = "cratesio_index"]
    Table,
    Id,
    Name,
    Vers,
    Deps,
    Cksum,
    Features,
    Features2,
    Yanked,
    Links,
    V,
    #[iden = "crates_io_fk"]
    CratesIoFk,
    Pubtime,
}

#[derive(Iden)]
enum CratesIoMetaIden {
    #[iden = "cratesio_meta"]
    Table,
    Id,
    Version,
    Downloads,
    #[iden = "crates_io_fk"]
    CratesIoFk,
    Documentation,
}

#[derive(Iden)]
enum DocQueueIden {
    #[iden = "doc_queue"]
    Table,
    Id,
    Krate,
    Version,
    Path,
}

#[derive(Iden)]
enum WebhookIden {
    #[iden = "webhook"]
    Table,
    Id,
    Event,
    #[iden = "callback_url"]
    CallbackUrl,
    Name,
}

#[derive(Iden)]
enum WebhookQueueIden {
    #[iden = "webhook_queue"]
    Table,
    Id,
    #[iden = "webhook_fk"]
    WebhookFk,
    Payload,
    #[iden = "last_attempt"]
    LastAttempt,
    #[iden = "next_attempt"]
    NextAttempt,
}
