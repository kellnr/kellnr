use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::version::Version;
use kellnr_entity::{crate_meta, cratesio_crate, cratesio_meta, krate};
use kellnr_migration::iden::{CrateIden, CrateMetaIden, CratesIoIden, CratesIoMetaIden};
use sea_orm::sea_query::{Cond, Expr, Order, Query};
use sea_orm::{ColumnTrait, EntityTrait, ExprTrait, FromQueryResult, QueryFilter};

use super::{Database, saturating_i64};
use crate::error::DbError;
use crate::provider::{CrateVisibility, DbResult};

impl Database {
    pub(super) async fn get_total_unique_cached_crates(&self) -> DbResult<u64> {
        self.count(
            CratesIoIden::Table,
            CratesIoIden::Id,
            DbError::FailedToCountCrates,
        )
        .await
    }

    pub(super) async fn get_total_cached_crate_versions(&self) -> DbResult<u64> {
        self.count(
            CratesIoMetaIden::Table,
            CratesIoMetaIden::Id,
            DbError::FailedToCountCrateVersions,
        )
        .await
    }

    pub(super) async fn get_total_cached_downloads(&self) -> DbResult<u64> {
        self.sum(
            CratesIoIden::Table,
            CratesIoIden::TotalDownloads,
            DbError::FailedToCountTotalDownloads,
        )
        .await
    }

    pub(super) async fn increase_download_counter(
        &self,
        crate_name: &CanonicalName,
        crate_version: &Version,
    ) -> DbResult<()> {
        self.in_transaction(async |txn| {
            krate::Entity::update_many()
                .col_expr(
                    krate::Column::TotalDownloads,
                    Expr::col(krate::Column::TotalDownloads).add(1),
                )
                .filter(krate::Column::Name.eq(crate_name))
                .exec(txn)
                .await?;

            let crate_id = krate::Entity::find()
                .filter(krate::Column::Name.eq(crate_name))
                .one(txn)
                .await?
                .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?
                .id;

            crate_meta::Entity::update_many()
                .col_expr(
                    crate_meta::Column::Downloads,
                    Expr::col(crate_meta::Column::Downloads).add(1),
                )
                .filter(
                    Cond::all()
                        .add(crate_meta::Column::Version.eq(crate_version))
                        .add(crate_meta::Column::CrateFk.eq(crate_id)),
                )
                .exec(txn)
                .await?;

            Ok(())
        })
        .await
    }

    pub(super) async fn increase_cached_download_counter(
        &self,
        crate_name: &CanonicalName,
        crate_version: &Version,
    ) -> DbResult<()> {
        self.in_transaction(async |txn| {
            cratesio_crate::Entity::update_many()
                .col_expr(
                    cratesio_crate::Column::TotalDownloads,
                    Expr::col(cratesio_crate::Column::TotalDownloads).add(1),
                )
                .filter(cratesio_crate::Column::Name.eq(crate_name))
                .exec(txn)
                .await?;

            let crate_id = cratesio_crate::Entity::find()
                .filter(cratesio_crate::Column::Name.eq(crate_name))
                .one(txn)
                .await?
                .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?
                .id;

            cratesio_meta::Entity::update_many()
                .col_expr(
                    cratesio_meta::Column::Downloads,
                    Expr::col(cratesio_meta::Column::Downloads).add(1),
                )
                .filter(
                    Cond::all()
                        .add(cratesio_meta::Column::Version.eq(crate_version))
                        .add(cratesio_meta::Column::CratesIoFk.eq(crate_id)),
                )
                .exec(txn)
                .await?;

            Ok(())
        })
        .await
    }

    pub(super) async fn increase_download_counter_by(
        &self,
        crate_name: &CanonicalName,
        crate_version: &Version,
        count: u64,
    ) -> DbResult<()> {
        self.in_transaction(async |txn| {
            krate::Entity::update_many()
                .col_expr(
                    krate::Column::TotalDownloads,
                    Expr::col(krate::Column::TotalDownloads).add(saturating_i64(count)),
                )
                .filter(krate::Column::Name.eq(crate_name))
                .exec(txn)
                .await?;

            let crate_id = krate::Entity::find()
                .filter(krate::Column::Name.eq(crate_name))
                .one(txn)
                .await?
                .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?
                .id;

            crate_meta::Entity::update_many()
                .col_expr(
                    crate_meta::Column::Downloads,
                    Expr::col(crate_meta::Column::Downloads).add(saturating_i64(count)),
                )
                .filter(
                    Cond::all()
                        .add(crate_meta::Column::Version.eq(crate_version))
                        .add(crate_meta::Column::CrateFk.eq(crate_id)),
                )
                .exec(txn)
                .await?;

            Ok(())
        })
        .await
    }

    pub(super) async fn increase_cached_download_counter_by(
        &self,
        crate_name: &CanonicalName,
        crate_version: &Version,
        count: u64,
    ) -> DbResult<()> {
        self.in_transaction(async |txn| {
            cratesio_crate::Entity::update_many()
                .col_expr(
                    cratesio_crate::Column::TotalDownloads,
                    Expr::col(cratesio_crate::Column::TotalDownloads).add(saturating_i64(count)),
                )
                .filter(cratesio_crate::Column::Name.eq(crate_name))
                .exec(txn)
                .await?;

            let crate_id = cratesio_crate::Entity::find()
                .filter(cratesio_crate::Column::Name.eq(crate_name))
                .one(txn)
                .await?
                .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?
                .id;

            cratesio_meta::Entity::update_many()
                .col_expr(
                    cratesio_meta::Column::Downloads,
                    Expr::col(cratesio_meta::Column::Downloads).add(saturating_i64(count)),
                )
                .filter(
                    Cond::all()
                        .add(cratesio_meta::Column::Version.eq(crate_version))
                        .add(cratesio_meta::Column::CratesIoFk.eq(crate_id)),
                )
                .exec(txn)
                .await?;

            Ok(())
        })
        .await
    }

    pub(super) async fn get_total_unique_crates(&self) -> DbResult<u64> {
        self.count(
            CrateIden::Table,
            CrateIden::Id,
            DbError::FailedToCountCrates,
        )
        .await
    }

    pub(super) async fn get_total_crate_versions(&self) -> DbResult<u64> {
        self.count(
            CrateMetaIden::Table,
            CrateMetaIden::Id,
            DbError::FailedToCountCrateVersions,
        )
        .await
    }

    pub(super) async fn get_total_downloads(&self) -> DbResult<u64> {
        self.sum(
            CrateIden::Table,
            CrateIden::TotalDownloads,
            DbError::FailedToCountTotalDownloads,
        )
        .await
    }

    pub(super) async fn get_top_crates_downloads(
        &self,
        top: u64,
        visibility: &CrateVisibility,
    ) -> DbResult<Vec<(String, u64)>> {
        #[derive(Debug, PartialEq, FromQueryResult)]
        struct SelectResult {
            original_name: String,
            total_downloads: i64,
        }

        let mut query = Query::select()
            .columns(vec![CrateIden::OriginalName, CrateIden::TotalDownloads])
            .from(CrateIden::Table)
            .order_by(CrateIden::TotalDownloads, Order::Desc)
            .limit(top)
            .to_owned();
        if let Some(condition) = Self::crate_visibility_condition(visibility) {
            query.cond_where(condition);
        }
        let stmt = self.db_con.get_database_backend().build(&query);

        Ok(SelectResult::find_by_statement(stmt)
            .all(&self.db_con)
            .await?
            .into_iter()
            .map(|x| {
                (
                    x.original_name,
                    u64::try_from(x.total_downloads).unwrap_or(0),
                )
            })
            .collect())
    }
}
