use chrono::Utc;
use kellnr_entity::{toolchain, toolchain_component, toolchain_target};
use sea_orm::query::QueryOrder;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, Set};

use super::{DB_DATE_FORMAT, Database};
use crate::ToolchainTargetStatus;
use crate::error::DbError;
use crate::provider::{
    ChannelInfo, DbResult, ToolchainComponentInfo, ToolchainTargetInfo, ToolchainWithTargets,
};

impl Database {
    /// Loads targets (with their components) for a toolchain and returns a `ToolchainWithTargets`.
    async fn toolchain_with_targets(&self, tc: toolchain::Model) -> DbResult<ToolchainWithTargets> {
        let targets = toolchain_target::Entity::find()
            .filter(toolchain_target::Column::ToolchainFk.eq(tc.id))
            .all(&self.db_con)
            .await?;

        let mut target_infos = Vec::with_capacity(targets.len());
        for t in targets {
            let components = toolchain_component::Entity::find()
                .filter(toolchain_component::Column::ToolchainTargetFk.eq(t.id))
                .all(&self.db_con)
                .await?;
            target_infos.push(ToolchainTargetInfo {
                id: t.id,
                target: t.target,
                storage_path: t.storage_path,
                hash: t.hash,
                size: t.size,
                status: ToolchainTargetStatus::from_db(&t.status),
                components: components
                    .into_iter()
                    .map(ToolchainComponentInfo::from)
                    .collect(),
            });
        }

        Ok(ToolchainWithTargets {
            id: tc.id,
            name: tc.name,
            version: tc.version,
            date: tc.date,
            channel: tc.channel,
            created: tc.created,
            targets: target_infos,
        })
    }

    /// Looks up a toolchain by name and version; returns `None` if not found.
    async fn get_toolchain_by_name_version(
        &self,
        name: &str,
        version: &str,
    ) -> DbResult<Option<toolchain::Model>> {
        toolchain::Entity::find()
            .filter(toolchain::Column::Name.eq(name))
            .filter(toolchain::Column::Version.eq(version))
            .one(&self.db_con)
            .await
            .map_err(Into::into)
    }

    pub(super) async fn add_toolchain(
        &self,
        name: &str,
        version: &str,
        date: &str,
        channel: Option<String>,
    ) -> DbResult<i64> {
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();

        let model = toolchain::ActiveModel {
            id: ActiveValue::NotSet,
            name: Set(name.to_string()),
            version: Set(version.to_string()),
            date: Set(date.to_string()),
            channel: Set(channel),
            created: Set(created),
        };

        let result = model.insert(&self.db_con).await?;
        Ok(result.id)
    }

    pub(super) async fn add_toolchain_target(
        &self,
        toolchain_id: i64,
        target: &str,
        storage_path: &str,
        hash: &str,
        size: i64,
    ) -> DbResult<i64> {
        let model = toolchain_target::ActiveModel {
            id: ActiveValue::NotSet,
            toolchain_fk: Set(toolchain_id),
            target: Set(target.to_string()),
            storage_path: Set(storage_path.to_string()),
            hash: Set(hash.to_string()),
            size: Set(size),
            status: Set(ToolchainTargetStatus::Processing.as_str().to_string()),
        };

        let result = model.insert(&self.db_con).await?;
        Ok(result.id)
    }

    pub(super) async fn set_target_status(
        &self,
        target_id: i64,
        status: ToolchainTargetStatus,
    ) -> DbResult<()> {
        toolchain_target::Entity::update_many()
            .col_expr(
                toolchain_target::Column::Status,
                Expr::value(status.as_str()),
            )
            .filter(toolchain_target::Column::Id.eq(target_id))
            .exec(&self.db_con)
            .await?;
        Ok(())
    }

    pub(super) async fn fail_processing_toolchain_targets(&self) -> DbResult<u64> {
        let result = toolchain_target::Entity::update_many()
            .col_expr(
                toolchain_target::Column::Status,
                Expr::value(ToolchainTargetStatus::Failed.as_str()),
            )
            .filter(toolchain_target::Column::Status.eq(ToolchainTargetStatus::Processing.as_str()))
            .exec(&self.db_con)
            .await?;
        Ok(result.rows_affected)
    }

    pub(super) async fn add_toolchain_component(
        &self,
        target_id: i64,
        name: &str,
        storage_path: &str,
        hash: &str,
        size: i64,
    ) -> DbResult<()> {
        let model = toolchain_component::ActiveModel {
            id: ActiveValue::NotSet,
            toolchain_target_fk: Set(target_id),
            name: Set(name.to_string()),
            storage_path: Set(storage_path.to_string()),
            hash: Set(hash.to_string()),
            size: Set(size),
        };

        model.insert(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn is_toolchain_component_stored_at(
        &self,
        storage_path: &str,
    ) -> DbResult<bool> {
        Ok(toolchain_component::Entity::find()
            .filter(toolchain_component::Column::StoragePath.eq(storage_path))
            .one(&self.db_con)
            .await?
            .is_some())
    }

    pub(super) async fn get_toolchain_by_channel(
        &self,
        channel: &str,
    ) -> DbResult<Option<ToolchainWithTargets>> {
        let tc = toolchain::Entity::find()
            .filter(toolchain::Column::Channel.eq(channel))
            .one(&self.db_con)
            .await?;

        Ok(match tc {
            Some(tc) => Some(self.toolchain_with_targets(tc).await?),
            None => None,
        })
    }

    pub(super) async fn get_toolchain_by_version(
        &self,
        name: &str,
        version: &str,
    ) -> DbResult<Option<ToolchainWithTargets>> {
        Ok(
            match self.get_toolchain_by_name_version(name, version).await? {
                Some(tc) => Some(self.toolchain_with_targets(tc).await?),
                None => None,
            },
        )
    }

    pub(super) async fn list_toolchains(&self) -> DbResult<Vec<ToolchainWithTargets>> {
        let toolchains = toolchain::Entity::find()
            .order_by_desc(toolchain::Column::Created)
            .all(&self.db_con)
            .await?;

        let mut result = Vec::with_capacity(toolchains.len());

        for tc in toolchains {
            result.push(self.toolchain_with_targets(tc).await?);
        }

        Ok(result)
    }

    pub(super) async fn delete_toolchain(&self, name: &str, version: &str) -> DbResult<()> {
        toolchain::Entity::delete_many()
            .filter(toolchain::Column::Name.eq(name))
            .filter(toolchain::Column::Version.eq(version))
            .exec(&self.db_con)
            .await?;

        Ok(())
    }

    pub(super) async fn delete_toolchain_target(
        &self,
        name: &str,
        version: &str,
        target: &str,
    ) -> DbResult<()> {
        if let Some(tc) = self.get_toolchain_by_name_version(name, version).await? {
            toolchain_target::Entity::delete_many()
                .filter(toolchain_target::Column::ToolchainFk.eq(tc.id))
                .filter(toolchain_target::Column::Target.eq(target))
                .exec(&self.db_con)
                .await?;
        }

        Ok(())
    }

    pub(super) async fn set_channel(
        &self,
        channel: &str,
        name: &str,
        version: &str,
    ) -> DbResult<()> {
        // In one transaction, so the channel is never lost or on two toolchains
        self.in_transaction(async |txn| {
            let tc = toolchain::Entity::find()
                .filter(toolchain::Column::Name.eq(name))
                .filter(toolchain::Column::Version.eq(version))
                .one(txn)
                .await?
                .ok_or_else(|| DbError::ToolchainNotFound(name.to_string(), version.to_string()))?;

            // First, clear the channel from any other toolchain that has it
            toolchain::Entity::update_many()
                .col_expr(
                    toolchain::Column::Channel,
                    Expr::value(Option::<String>::None),
                )
                .filter(toolchain::Column::Channel.eq(channel))
                .exec(txn)
                .await?;

            // Then set the channel on the target toolchain
            let mut model: toolchain::ActiveModel = tc.into();
            model.channel = Set(Some(channel.to_string()));
            model.update(txn).await?;

            Ok(())
        })
        .await
    }

    pub(super) async fn get_channels(&self) -> DbResult<Vec<ChannelInfo>> {
        let toolchains = toolchain::Entity::find()
            .filter(toolchain::Column::Channel.is_not_null())
            .all(&self.db_con)
            .await?;

        Ok(toolchains
            .into_iter()
            .filter_map(|tc| {
                tc.channel.map(|channel| ChannelInfo {
                    name: channel,
                    version: tc.version,
                    date: tc.date,
                })
            })
            .collect())
    }
}
