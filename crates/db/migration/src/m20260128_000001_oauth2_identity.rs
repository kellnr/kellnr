//! Migration for OAuth2/OIDC identity tables
//!
//! This migration adds tables for:
//! - oauth2_identity: Links OAuth2 provider identities to local Kellnr users
//! - oauth2_state: Temporary storage for PKCE/CSRF state during OAuth2 auth flow

use sea_orm_migration::prelude::*;

use crate::iden::{OAuth2IdentityIden, OAuth2StateIden, UserIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // oauth2_identity table - links OAuth2 identities to local users
        manager
            .create_table(
                Table::create()
                    .table(OAuth2IdentityIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuth2IdentityIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OAuth2IdentityIden::UserFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuth2IdentityIden::ProviderIssuer)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuth2IdentityIden::Subject)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OAuth2IdentityIden::Email).text())
                    .col(
                        ColumnDef::new(OAuth2IdentityIden::Created)
                            .text()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("oauth2_identity_user_fk")
                            .from(OAuth2IdentityIden::Table, OAuth2IdentityIden::UserFk)
                            .to(UserIden::Table, UserIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique index on (provider_issuer, subject) - ensures one identity per provider/user
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_oauth2_identity_provider_subject")
                    .table(OAuth2IdentityIden::Table)
                    .col(OAuth2IdentityIden::ProviderIssuer)
                    .col(OAuth2IdentityIden::Subject)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on user_fk for efficient lookups
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_oauth2_identity_user_fk")
                    .table(OAuth2IdentityIden::Table)
                    .col(OAuth2IdentityIden::UserFk)
                    .to_owned(),
            )
            .await?;

        // oauth2_state table - temporary storage for OAuth2 auth flow
        manager
            .create_table(
                Table::create()
                    .table(OAuth2StateIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuth2StateIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OAuth2StateIden::State)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(OAuth2StateIden::PkceVerifier)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OAuth2StateIden::Nonce).text().not_null())
                    .col(ColumnDef::new(OAuth2StateIden::Created).text().not_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order
        manager
            .drop_table(Table::drop().table(OAuth2StateIden::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(OAuth2IdentityIden::Table).to_owned())
            .await?;

        Ok(())
    }
}
