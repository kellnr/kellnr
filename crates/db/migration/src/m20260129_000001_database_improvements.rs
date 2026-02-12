//! Database improvements migration
//!
//! This migration adds:
//! - `created` column to the user table for tracking user creation time
//! - Performance indices for common query patterns

use sea_orm_migration::prelude::*;

use crate::iden::{CrateIden, OAuth2StateIden, OwnerIden, SessionIden, UserIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ========================================
        // 1. Add `created` column to user table
        // ========================================
        manager
            .alter_table(
                Table::alter()
                    .table(UserIden::Table)
                    .add_column(
                        ColumnDef::new(UserIden::Created)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        // ========================================
        // 2. Add performance indices
        // ========================================

        // Index on krate.name for crate lookups by name
        // Note: krate.name has unique_key which creates implicit index in SQLite,
        // but explicit index ensures consistent behavior across databases
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_krate_name")
                    .table(CrateIden::Table)
                    .col(CrateIden::Name)
                    .to_owned(),
            )
            .await?;

        // Index on owner.crate_fk for efficient ownership queries
        // (complements the existing composite unique index idx-owner)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_owner_crate_fk")
                    .table(OwnerIden::Table)
                    .col(OwnerIden::CrateFk)
                    .to_owned(),
            )
            .await?;

        // Index on session.token for session validation
        // Note: session.token has unique_key, but explicit index ensures PostgreSQL performance
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_session_token")
                    .table(SessionIden::Table)
                    .col(SessionIden::Token)
                    .to_owned(),
            )
            .await?;

        // Index on oauth2_state.state for OAuth2 flow lookups
        // Note: oauth2_state.state has unique_key, explicit index for consistency
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_oauth2_state_state")
                    .table(OAuth2StateIden::Table)
                    .col(OAuth2StateIden::State)
                    .to_owned(),
            )
            .await?;

        // Index on oauth2_state.created for TTL cleanup queries
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_oauth2_state_created")
                    .table(OAuth2StateIden::Table)
                    .col(OAuth2StateIden::Created)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indices in reverse order
        manager
            .drop_index(
                Index::drop()
                    .name("idx_oauth2_state_created")
                    .table(OAuth2StateIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_oauth2_state_state")
                    .table(OAuth2StateIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_session_token")
                    .table(SessionIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_owner_crate_fk")
                    .table(OwnerIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_krate_name")
                    .table(CrateIden::Table)
                    .to_owned(),
            )
            .await?;

        // Drop created column from user table
        manager
            .alter_table(
                Table::alter()
                    .table(UserIden::Table)
                    .drop_column(UserIden::Created)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
