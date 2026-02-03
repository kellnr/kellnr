mod operations;
pub mod test_utils;

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use kellnr_common::crate_data::{CrateData, CrateRegistryDep, CrateVersionData};
use kellnr_common::crate_overview::CrateOverview;
use kellnr_common::cratesio_prefetch_msg::{CratesioPrefetchMsg, UpdateData};
use kellnr_common::index_metadata::{IndexDep, IndexMetadata};
use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::original_name::OriginalName;
use kellnr_common::prefetch::Prefetch;
use kellnr_common::publish_metadata::PublishMetadata;
use kellnr_common::version::Version;
use kellnr_common::webhook::{Webhook, WebhookEvent, WebhookQueue};
use kellnr_entity::prelude::*;
use kellnr_entity::{
    auth_token, crate_author, crate_author_to_crate, crate_category, crate_category_to_crate,
    crate_group, crate_index, crate_keyword, crate_keyword_to_crate, crate_meta, crate_user,
    cratesio_crate, cratesio_index, cratesio_meta, doc_queue, group, group_user, krate,
    oauth2_identity, oauth2_state, owner, session, toolchain, toolchain_target, user, webhook,
    webhook_queue,
};
use kellnr_migration::iden::{
    AuthTokenIden, CrateIden, CrateMetaIden, CratesIoIden, CratesIoMetaIden, GroupIden,
};
use sea_orm::entity::prelude::Uuid;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::query::{QueryOrder, QuerySelect, TransactionTrait};
use sea_orm::sea_query::{Alias, Cond, Expr, Iden, JoinType, Order, Query, UnionType};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, ExprTrait,
    FromQueryResult, ModelTrait, QueryFilter, RelationTrait, Set,
};

use crate::error::DbError;
use crate::password::{generate_salt, hash_pwd, hash_token};
use crate::provider::{
    ChannelInfo, DbResult, OAuth2StateData, PrefetchState, ToolchainWithTargets,
};
use crate::tables::init_database;
use crate::{
    AuthToken, ConString, CrateMeta, CrateSummary, DbProvider, DocQueueEntry, Group, User,
};

const DB_DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub struct Database {
    db_con: DatabaseConnection,
}

impl Database {
    pub fn existing(db_con: DatabaseConnection) -> Self {
        Self { db_con }
    }

    pub async fn new(con: &ConString, max_con: u32) -> Result<Self, DbError> {
        let db_con = init_database(con, max_con)
            .await
            .map_err(|e| DbError::InitializationError(e.to_string()))?;

        if operations::no_user_exists(&db_con).await? {
            operations::insert_admin_credentials(&db_con, con).await?;
        }

        Ok(Self { db_con })
    }

    /// Looks up a user by name; returns the entity model or [`DbError::UserNotFound`].
    async fn get_user_model(&self, name: &str) -> DbResult<user::Model> {
        user::Entity::find()
            .filter(user::Column::Name.eq(name))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::UserNotFound(name.to_owned()))
    }

    /// Looks up a krate by normalized name; returns the entity model or [`DbError::CrateNotFound`].
    async fn get_krate_model(&self, crate_name: &NormalizedName) -> DbResult<krate::Model> {
        krate::Entity::find()
            .filter(krate::Column::Name.eq(&**crate_name))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))
    }

    /// Looks up a group by name; returns the entity model or [`DbError::GroupNotFound`].
    async fn get_group_model(&self, name: &str) -> DbResult<group::Model> {
        group::Entity::find()
            .filter(group::Column::Name.eq(name))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::GroupNotFound(name.to_owned()))
    }

    /// Loads targets for a toolchain and returns a `ToolchainWithTargets`.
    async fn toolchain_with_targets(&self, tc: toolchain::Model) -> DbResult<ToolchainWithTargets> {
        let targets = toolchain_target::Entity::find()
            .filter(toolchain_target::Column::ToolchainFk.eq(tc.id))
            .all(&self.db_con)
            .await?;
        Ok(ToolchainWithTargets::from_model(tc, targets))
    }

    /// Looks up a crate index by name and version; returns the entity model or [`DbError::CrateIndexNotFound`].
    async fn get_crate_index_model(
        &self,
        crate_name: &NormalizedName,
        version: &Version,
    ) -> DbResult<crate_index::Model> {
        crate_index::Entity::find()
            .filter(crate_index::Column::Name.eq(&**crate_name))
            .filter(crate_index::Column::Vers.eq(&**version))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateIndexNotFound(crate_name.to_string(), version.to_string()))
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

    /// Looks up a `crate_group` by crate name and group name; returns `None` if not found.
    async fn get_crate_group_by_name_and_group(
        &self,
        crate_name: &NormalizedName,
        group: &str,
    ) -> DbResult<Option<crate_group::Model>> {
        crate_group::Entity::find()
            .join(JoinType::InnerJoin, crate_group::Relation::Krate.def())
            .join(JoinType::InnerJoin, crate_group::Relation::Group.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(&**crate_name))
                    .add(group::Column::Name.eq(group)),
            )
            .one(&self.db_con)
            .await
            .map_err(Into::into)
    }

    /// Looks up a `group_user` by group name and user name; returns `None` if not found.
    async fn get_group_user_by_group_and_user(
        &self,
        group_name: &str,
        user: &str,
    ) -> DbResult<Option<group_user::Model>> {
        group_user::Entity::find()
            .join(JoinType::InnerJoin, group_user::Relation::Group.def())
            .join(JoinType::InnerJoin, group_user::Relation::User.def())
            .filter(
                Cond::all()
                    .add(group::Column::Name.eq(group_name))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await
            .map_err(Into::into)
    }

    /// Looks up a `crate_user` by crate name and user name; returns `None` if not found.
    async fn get_crate_user_by_crate_and_user(
        &self,
        crate_name: &NormalizedName,
        user: &str,
    ) -> DbResult<Option<crate_user::Model>> {
        crate_user::Entity::find()
            .join(JoinType::InnerJoin, crate_user::Relation::Krate.def())
            .join(JoinType::InnerJoin, crate_user::Relation::User.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(&**crate_name))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await
            .map_err(Into::into)
    }

    /// Executes a count query `SELECT COUNT(id_column) FROM table` and returns the count as u64.
    async fn count<T>(&self, table: T, id_column: T, error: DbError) -> DbResult<u64>
    where
        T: Iden + Copy,
    {
        #[derive(Debug, PartialEq, FromQueryResult)]
        struct CountResult {
            count: Option<i64>,
        }

        let stmt = Query::select()
            .expr_as(Expr::col((table, id_column)).count(), Alias::new("count"))
            .from(table)
            .to_owned();
        let statement = self.db_con.get_database_backend().build(&stmt);
        let Some(result) = CountResult::find_by_statement(statement)
            .one(&self.db_con)
            .await?
        else {
            return Err(error);
        };
        result
            .count
            .and_then(|v| u64::try_from(v).ok())
            .ok_or(error)
    }
}

#[async_trait]
impl DbProvider for Database {
    async fn get_total_unique_cached_crates(&self) -> DbResult<u64> {
        self.count(
            CratesIoIden::Table,
            CratesIoIden::Id,
            DbError::FailedToCountCrates,
        )
        .await
    }

    async fn get_total_cached_crate_versions(&self) -> DbResult<u64> {
        self.count(
            CratesIoMetaIden::Table,
            CratesIoMetaIden::Id,
            DbError::FailedToCountCrateVersions,
        )
        .await
    }

    async fn get_total_cached_downloads(&self) -> DbResult<u64> {
        #[derive(FromQueryResult)]
        struct Model {
            total_downloads: i64,
        }

        let total_downloads = cratesio_crate::Entity::find()
            .select_only()
            .column(cratesio_crate::Column::TotalDownloads)
            .into_model::<Model>()
            .all(&self.db_con)
            .await?;

        Ok(total_downloads
            .iter()
            .map(|m| m.total_downloads as u64)
            .sum())
    }

    async fn authenticate_user(&self, name: &str, pwd: &str) -> DbResult<User> {
        let user = self.get_user(name).await?;

        if hash_pwd(pwd, &user.salt) == user.pwd {
            Ok(user)
        } else {
            Err(DbError::PasswordMismatch)
        }
    }

    async fn increase_download_counter(
        &self,
        crate_name: &NormalizedName,
        crate_version: &Version,
    ) -> DbResult<()> {
        let krate = self.get_krate_model(crate_name).await?;
        let crate_id = krate.id;
        let crate_total_downloads = krate.total_downloads;

        // Update the total downloads for the whole crate (all versions)
        let mut k: krate::ActiveModel = krate.into();
        k.total_downloads = Set(crate_total_downloads + 1);
        k.update(&self.db_con).await?;

        // Update the downloads for the specific version
        crate_meta::Entity::update_many()
            .col_expr(
                crate_meta::Column::Downloads,
                Expr::col(crate_meta::Column::Downloads).add(1),
            )
            .filter(
                Cond::all()
                    .add(crate_meta::Column::Version.eq(crate_version.to_string()))
                    .add(crate_meta::Column::CrateFk.eq(crate_id)),
            )
            .exec(&self.db_con)
            .await?;

        Ok(())
    }

    async fn increase_cached_download_counter(
        &self,
        crate_name: &NormalizedName,
        crate_version: &Version,
    ) -> DbResult<()> {
        let krate: cratesio_crate::Model = cratesio_crate::Entity::find()
            .filter(cratesio_crate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?;
        let crate_id = krate.id;
        let crate_total_downloads = krate.total_downloads;

        // Update the total downloads for the whole crate (all versions)
        let mut k: cratesio_crate::ActiveModel = krate.into();
        k.total_downloads = Set(crate_total_downloads + 1);
        k.update(&self.db_con).await?;

        // Update the downloads for the specific version
        cratesio_meta::Entity::update_many()
            .col_expr(
                cratesio_meta::Column::Downloads,
                Expr::col(cratesio_meta::Column::Downloads).add(1),
            )
            .filter(
                Cond::all()
                    .add(cratesio_meta::Column::Version.eq(crate_version.to_string()))
                    .add(cratesio_meta::Column::CratesIoFk.eq(crate_id)),
            )
            .exec(&self.db_con)
            .await?;

        Ok(())
    }

    async fn get_last_updated_crate(&self) -> DbResult<Option<(OriginalName, Version)>> {
        let krate = krate::Entity::find()
            .order_by_desc(krate::Column::LastUpdated)
            .one(&self.db_con)
            .await?;

        if let Some(krate) = krate {
            // SAFETY: Unchecked is ok, as only valid crate names are inserted into the database
            let name = OriginalName::from_unchecked(krate.original_name);
            // SAFETY: Unchecked is ok, as only valid versions are inserted into the database
            let version = Version::from_unchecked_str(&krate.max_version);
            Ok(Some((name, version)))
        } else {
            Ok(None)
        }
    }

    async fn validate_session(&self, session_token: &str) -> DbResult<(String, bool)> {
        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::Session.def())
            .filter(session::Column::Token.eq(session_token))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::SessionNotFound)?;

        Ok((u.name, u.is_admin))
    }

    async fn add_session_token(&self, name: &str, session_token: &str) -> DbResult<()> {
        let user = self.get_user(name).await?;
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();

        let s = session::ActiveModel {
            token: Set(session_token.to_owned()),
            created: Set(created),
            user_fk: Set(user.id as i64),
            ..Default::default()
        };

        s.insert(&self.db_con).await?;
        Ok(())
    }

    async fn add_crate_user(&self, crate_name: &NormalizedName, user: &str) -> DbResult<()> {
        let user_fk = self.get_user_model(user).await?.id;
        let crate_fk = self.get_krate_model(crate_name).await?.id;

        let u = crate_user::ActiveModel {
            user_fk: Set(user_fk),
            crate_fk: Set(crate_fk),
            ..Default::default()
        };

        CrateUser::insert(u).exec(&self.db_con).await?;
        Ok(())
    }

    async fn add_crate_group(&self, crate_name: &NormalizedName, group: &str) -> DbResult<()> {
        let group_fk = self.get_group_model(group).await?.id;
        let crate_fk = self.get_krate_model(crate_name).await?.id;

        let u = crate_group::ActiveModel {
            group_fk: Set(group_fk),
            crate_fk: Set(crate_fk),
            ..Default::default()
        };

        CrateGroup::insert(u).exec(&self.db_con).await?;
        Ok(())
    }

    async fn add_group_user(&self, group_name: &str, user: &str) -> DbResult<()> {
        let user_fk = self.get_user_model(user).await?.id;
        let group_fk = self.get_group_model(group_name).await?.id;

        let u = group_user::ActiveModel {
            user_fk: Set(user_fk),
            group_fk: Set(group_fk),
            ..Default::default()
        };

        GroupUser::insert(u).exec(&self.db_con).await?;
        Ok(())
    }

    async fn add_owner(&self, crate_name: &NormalizedName, owner: &str) -> DbResult<()> {
        let user_fk = self.get_user_model(owner).await?.id;
        let crate_fk = self.get_krate_model(crate_name).await?.id;

        let o = owner::ActiveModel {
            user_fk: Set(user_fk),
            crate_fk: Set(crate_fk),
            ..Default::default()
        };

        Owner::insert(o).exec(&self.db_con).await?;
        Ok(())
    }

    async fn is_download_restricted(&self, crate_name: &NormalizedName) -> DbResult<bool> {
        Ok(krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
            .is_some_and(|model| model.restricted_download))
    }

    async fn change_download_restricted(
        &self,
        crate_name: &NormalizedName,
        restricted: bool,
    ) -> DbResult<()> {
        let mut krate: krate::ActiveModel = self.get_krate_model(crate_name).await?.into();
        krate.restricted_download = Set(restricted);
        krate.update(&self.db_con).await?;
        Ok(())
    }

    async fn is_crate_user(&self, crate_name: &NormalizedName, user: &str) -> DbResult<bool> {
        Ok(self
            .get_crate_user_by_crate_and_user(crate_name, user)
            .await?
            .is_some())
    }

    async fn is_crate_group(&self, crate_name: &NormalizedName, group: &str) -> DbResult<bool> {
        Ok(self
            .get_crate_group_by_name_and_group(crate_name, group)
            .await?
            .is_some())
    }

    async fn is_crate_group_user(&self, crate_name: &NormalizedName, user: &str) -> DbResult<bool> {
        let user = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::GroupUser.def())
            .join(JoinType::InnerJoin, group_user::Relation::Group.def())
            .join(JoinType::InnerJoin, group::Relation::CrateGroup.def())
            .join(JoinType::InnerJoin, crate_group::Relation::Krate.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name.to_string()))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await?;
        Ok(user.is_some())
    }

    async fn is_group_user(&self, group_name: &str, user: &str) -> DbResult<bool> {
        Ok(self
            .get_group_user_by_group_and_user(group_name, user)
            .await?
            .is_some())
    }

    async fn is_owner(&self, crate_name: &NormalizedName, user: &str) -> DbResult<bool> {
        let owner = owner::Entity::find()
            .join(JoinType::InnerJoin, owner::Relation::Krate.def())
            .join(JoinType::InnerJoin, owner::Relation::User.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name.to_string()))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await?;

        Ok(owner.is_some())
    }

    async fn get_crate_id(&self, crate_name: &NormalizedName) -> DbResult<Option<i64>> {
        let id = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
            .map(|model| model.id);

        Ok(id)
    }

    async fn get_crate_owners(&self, crate_name: &NormalizedName) -> DbResult<Vec<User>> {
        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::Owner.def())
            .join(JoinType::InnerJoin, owner::Relation::Krate.def())
            .filter(Expr::col((CrateIden::Table, krate::Column::Name)).eq(crate_name.to_string()))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter().map(User::from).collect())
    }

    async fn get_crate_users(&self, crate_name: &NormalizedName) -> DbResult<Vec<User>> {
        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::CrateUser.def())
            .join(JoinType::InnerJoin, crate_user::Relation::Krate.def())
            .filter(Expr::col((CrateIden::Table, krate::Column::Name)).eq(crate_name.to_string()))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter().map(User::from).collect())
    }

    async fn get_crate_groups(&self, crate_name: &NormalizedName) -> DbResult<Vec<Group>> {
        let u = group::Entity::find()
            .join(JoinType::InnerJoin, group::Relation::CrateGroup.def())
            .join(JoinType::InnerJoin, crate_group::Relation::Krate.def())
            .filter(Expr::col((CrateIden::Table, krate::Column::Name)).eq(crate_name.to_string()))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter().map(Group::from).collect())
    }

    async fn get_group_users(&self, group_name: &str) -> DbResult<Vec<User>> {
        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::GroupUser.def())
            .join(JoinType::InnerJoin, group_user::Relation::Group.def())
            .filter(Expr::col((GroupIden::Table, group::Column::Name)).eq(group_name.to_string()))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter().map(User::from).collect())
    }

    async fn get_crate_versions(&self, crate_name: &NormalizedName) -> DbResult<Vec<Version>> {
        let u = crate_meta::Entity::find()
            .join(JoinType::InnerJoin, crate_meta::Relation::Krate.def())
            .filter(Expr::col((CrateIden::Table, krate::Column::Name)).eq(crate_name.to_string()))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter()
            .map(|meta| Version::from_unchecked_str(&meta.version))
            .collect())
    }

    async fn delete_session_token(&self, session_token: &str) -> DbResult<()> {
        if let Some(s) = session::Entity::find()
            .filter(session::Column::Token.eq(session_token))
            .one(&self.db_con)
            .await?
        {
            s.delete(&self.db_con).await?;
        }

        Ok(())
    }

    async fn delete_user(&self, user_name: &str) -> DbResult<()> {
        self.get_user_model(user_name)
            .await?
            .delete(&self.db_con)
            .await?;
        Ok(())
    }

    async fn delete_group(&self, group_name: &str) -> DbResult<()> {
        self.get_group_model(group_name)
            .await?
            .delete(&self.db_con)
            .await?;
        Ok(())
    }

    async fn change_pwd(&self, user_name: &str, new_pwd: &str) -> DbResult<()> {
        let salt = generate_salt();
        let hashed = hash_pwd(new_pwd, &salt);

        let mut u: user::ActiveModel = self.get_user_model(user_name).await?.into();
        u.pwd = Set(hashed.clone());
        u.salt = Set(salt);

        u.update(&self.db_con).await?;
        Ok(())
    }

    async fn change_read_only_state(&self, user_name: &str, state: bool) -> DbResult<()> {
        let mut u: user::ActiveModel = self.get_user_model(user_name).await?.into();
        u.is_read_only = Set(state);

        u.update(&self.db_con).await?;
        Ok(())
    }

    async fn change_admin_state(&self, user_name: &str, state: bool) -> DbResult<()> {
        let mut u: user::ActiveModel = self.get_user_model(user_name).await?.into();
        u.is_admin = Set(state);

        u.update(&self.db_con).await?;
        Ok(())
    }

    async fn crate_version_exists(&self, crate_id: i64, version: &str) -> DbResult<bool> {
        let cm = crate_meta::Entity::find()
            .filter(
                Cond::all()
                    .add(crate_meta::Column::CrateFk.eq(crate_id))
                    .add(crate_meta::Column::Version.eq(version)),
            )
            .one(&self.db_con)
            .await?;

        Ok(cm.is_some())
    }

    async fn get_max_version_from_id(&self, crate_id: i64) -> DbResult<Version> {
        operations::get_max_version_from_id(&self.db_con, crate_id).await
    }

    async fn get_max_version_from_name(&self, crate_name: &NormalizedName) -> DbResult<Version> {
        let k = self.get_krate_model(crate_name).await?;
        let v = Version::try_from(&k.max_version)
            .map_err(|_| DbError::FailedToGetMaxVersionByName(crate_name.to_string()))?;
        Ok(v)
    }

    async fn update_max_version(&self, crate_id: i64, version: &Version) -> DbResult<()> {
        let krate = krate::Entity::find_by_id(crate_id)
            .one(&self.db_con)
            .await?
            .ok_or(DbError::CrateNotFoundWithId(crate_id))?;

        let mut k: krate::ActiveModel = krate.into();
        k.max_version = Set(version.to_string());
        k.update(&self.db_con).await?;

        Ok(())
    }

    async fn add_auth_token(&self, name: &str, token: &str, user: &str) -> DbResult<()> {
        let hashed_token = hash_token(token);
        let user = self.get_user_model(user).await?;

        let at = auth_token::ActiveModel {
            name: Set(name.to_owned()),
            token: Set(hashed_token.clone()),
            user_fk: Set(user.id),
            ..Default::default()
        };

        at.insert(&self.db_con).await?;

        Ok(())
    }

    async fn get_user_from_token(&self, token: &str) -> DbResult<User> {
        let token = hash_token(token);

        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::AuthToken.def())
            .filter(Expr::col((AuthTokenIden::Table, AuthTokenIden::Token)).eq(token))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::TokenNotFound)?;

        Ok(User::from(u))
    }

    async fn get_user(&self, name: &str) -> DbResult<User> {
        Ok(User::from(self.get_user_model(name).await?))
    }

    async fn get_group(&self, name: &str) -> DbResult<Group> {
        Ok(Group::from(self.get_group_model(name).await?))
    }

    async fn get_auth_tokens(&self, user_name: &str) -> DbResult<Vec<AuthToken>> {
        let at: Vec<auth_token::Model> = auth_token::Entity::find()
            .join(JoinType::InnerJoin, auth_token::Relation::User.def())
            .filter(user::Column::Name.eq(user_name))
            .all(&self.db_con)
            .await?;

        Ok(at.into_iter().map(AuthToken::from).collect())
    }

    async fn delete_auth_token(&self, id: i32) -> DbResult<()> {
        auth_token::Entity::delete_by_id(id as i64)
            .exec(&self.db_con)
            .await?;
        Ok(())
    }

    async fn delete_owner(&self, crate_name: &str, owner: &str) -> DbResult<()> {
        let owner = owner::Entity::find()
            .join(JoinType::InnerJoin, owner::Relation::Krate.def())
            .join(JoinType::InnerJoin, owner::Relation::User.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name))
                    .add(user::Column::Name.eq(owner)),
            )
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::OwnerNotFound(owner.to_string()))?;

        owner.delete(&self.db_con).await?;

        Ok(())
    }

    async fn delete_crate_user(&self, crate_name: &NormalizedName, user: &str) -> DbResult<()> {
        self.get_crate_user_by_crate_and_user(crate_name, user)
            .await?
            .ok_or_else(|| DbError::UserNotFound(user.to_string()))?
            .delete(&self.db_con)
            .await?;
        Ok(())
    }

    async fn delete_crate_group(&self, crate_name: &NormalizedName, group: &str) -> DbResult<()> {
        self.get_crate_group_by_name_and_group(crate_name, group)
            .await?
            .ok_or_else(|| DbError::GroupNotFound(group.to_string()))?
            .delete(&self.db_con)
            .await?;
        Ok(())
    }

    async fn delete_group_user(&self, group_name: &str, user: &str) -> DbResult<()> {
        self.get_group_user_by_group_and_user(group_name, user)
            .await?
            .ok_or_else(|| DbError::UserNotFound(user.to_string()))?
            .delete(&self.db_con)
            .await?;
        Ok(())
    }

    async fn add_user(
        &self,
        name: &str,
        pwd: &str,
        salt: &str,
        is_admin: bool,
        is_read_only: bool,
    ) -> DbResult<()> {
        let hashed_pwd = hash_pwd(pwd, salt);
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();

        let u = user::ActiveModel {
            name: Set(name.to_owned()),
            pwd: Set(hashed_pwd),
            salt: Set(salt.to_owned()),
            is_admin: Set(is_admin),
            is_read_only: Set(is_read_only),
            created: Set(created),
            ..Default::default()
        };

        u.insert(&self.db_con).await?;
        Ok(())
    }

    async fn add_group(&self, name: &str) -> DbResult<()> {
        let g = group::ActiveModel {
            name: Set(name.to_owned()),
            ..Default::default()
        };

        g.insert(&self.db_con).await?;
        Ok(())
    }

    async fn get_users(&self) -> DbResult<Vec<User>> {
        let users = user::Entity::find()
            .order_by_asc(user::Column::Name)
            .all(&self.db_con)
            .await?;

        Ok(users.into_iter().map(User::from).collect())
    }

    async fn get_groups(&self) -> DbResult<Vec<Group>> {
        let groups = group::Entity::find()
            .order_by_asc(group::Column::Name)
            .all(&self.db_con)
            .await?;

        Ok(groups.into_iter().map(Group::from).collect())
    }

    async fn get_total_unique_crates(&self) -> DbResult<u64> {
        self.count(
            CrateIden::Table,
            CrateIden::Id,
            DbError::FailedToCountCrates,
        )
        .await
    }

    async fn get_total_crate_versions(&self) -> DbResult<u64> {
        self.count(
            CrateMetaIden::Table,
            CrateMetaIden::Id,
            DbError::FailedToCountCrateVersions,
        )
        .await
    }

    async fn get_total_downloads(&self) -> DbResult<u64> {
        #[derive(FromQueryResult)]
        struct Model {
            total_downloads: i64,
        }

        let total_downloads = krate::Entity::find()
            .select_only()
            .column(krate::Column::TotalDownloads)
            .into_model::<Model>()
            .all(&self.db_con)
            .await?;

        Ok(total_downloads
            .iter()
            .map(|m| m.total_downloads as u64)
            .sum())
    }

    async fn get_top_crates_downloads(&self, top: u32) -> DbResult<Vec<(String, u64)>> {
        #[derive(Debug, PartialEq, FromQueryResult)]
        struct SelectResult {
            original_name: String,
            total_downloads: i64,
        }

        let stmt = Query::select()
            .columns(vec![CrateIden::OriginalName, CrateIden::TotalDownloads])
            .from(CrateIden::Table)
            .order_by(CrateIden::TotalDownloads, Order::Desc)
            .limit(top as u64)
            .to_owned();

        let builder = self.db_con.get_database_backend();
        let result = SelectResult::find_by_statement(builder.build(&stmt))
            .all(&self.db_con)
            .await?;

        Ok(result
            .iter()
            .map(|x| (x.original_name.clone(), x.total_downloads as u64))
            .collect())
    }

    async fn get_crate_summaries(&self) -> DbResult<Vec<CrateSummary>> {
        let krates = krate::Entity::find()
            .order_by(krate::Column::Name, Order::Asc)
            .all(&self.db_con)
            .await?;

        Ok(krates.into_iter().map(CrateSummary::from).collect())
    }

    async fn add_doc_queue(
        &self,
        krate: &NormalizedName,
        version: &Version,
        path: &Path,
    ) -> DbResult<()> {
        let s = doc_queue::ActiveModel {
            krate: Set(krate.to_string()),
            version: Set(version.to_string()),
            path: Set(path.to_string_lossy().to_string()),
            ..Default::default()
        };

        s.insert(&self.db_con).await?;
        Ok(())
    }

    async fn delete_doc_queue(&self, id: i64) -> DbResult<()> {
        DocQueue::delete_by_id(id).exec(&self.db_con).await?;
        Ok(())
    }

    async fn get_doc_queue(&self) -> DbResult<Vec<DocQueueEntry>> {
        let entities = DocQueue::find().all(&self.db_con).await?;

        Ok(entities.into_iter().map(DocQueueEntry::from).collect())
    }

    async fn delete_crate(&self, krate: &NormalizedName, version: &Version) -> DbResult<()> {
        let txn = self.db_con.begin().await?;

        // Delete the entry from the "crate_meta" table
        let crate_meta_version = crate_meta::Entity::find()
            .join(JoinType::InnerJoin, crate_meta::Relation::Krate.def())
            .filter(krate::Column::Name.eq(krate.to_string()))
            .filter(crate_meta::Column::Version.eq(version.to_string()))
            .one(&txn)
            .await?
            .ok_or_else(|| DbError::CrateMetaNotFound(krate.to_string(), version.to_string()))?;
        let crate_id = crate_meta_version.crate_fk;
        let current_max_version = operations::get_max_version_from_id(&txn, crate_id).await?;
        crate_meta_version.delete(&txn).await?;

        // Delete the crate index entry from "crate_index" table
        let crate_index_version = crate_index::Entity::find()
            .join(JoinType::InnerJoin, crate_index::Relation::Krate.def())
            .filter(krate::Column::Name.eq(krate.to_string()))
            .filter(crate_index::Column::Vers.eq(version.to_string()))
            .one(&txn)
            .await?
            .ok_or_else(|| DbError::CrateIndexNotFound(krate.to_string(), version.to_string()))?;
        crate_index_version.delete(&txn).await?;

        // If it was the last entry in the "crate_meta" table, delete the entry
        // in the "crate" table as well
        let crate_meta_rows = crate_meta::Entity::find()
            .join(JoinType::InnerJoin, crate_meta::Relation::Krate.def())
            .filter(krate::Column::Name.eq(krate.to_string()))
            .all(&txn)
            .await?;

        if crate_meta_rows.is_empty() {
            krate::Entity::delete_many()
                .filter(krate::Column::Name.eq(krate.to_string()))
                .exec(&txn)
                .await?;
        } else {
            let c = krate::Entity::find_by_id(crate_id)
                .one(&txn)
                .await?
                .ok_or(DbError::CrateNotFoundWithId(crate_id))?;
            let mut c: krate::ActiveModel = c.into();

            // Update the max. version if the deleted version was the max. version.
            if version == &current_max_version {
                let new_max_version = crate_meta_rows
                    .iter()
                    .map(|cm| Version::from_unchecked_str(&cm.version))
                    .max()
                    .unwrap(); // Safe to unwrap, as crate_meta_rows is not empty
                c.max_version = Set(new_max_version.to_string());
            }
            // Update the ETag value of the crate index.
            let etag = operations::compute_etag(&txn, krate, crate_id).await?;
            c.e_tag = Set(etag);
            c.update(&txn).await?;
        }

        txn.commit().await?;

        Ok(())
    }

    async fn get_crate_meta_list(&self, crate_name: &NormalizedName) -> DbResult<Vec<CrateMeta>> {
        let crate_meta = crate_meta::Entity::find()
            .join(JoinType::InnerJoin, crate_meta::Relation::Krate.def())
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .all(&self.db_con)
            .await?;

        let crate_meta = crate_meta
            .into_iter()
            .map(|cm| CrateMeta {
                name: crate_name.to_string(),
                id: cm.id,
                version: cm.version,
                created: cm.created,
                downloads: cm.downloads,
                crate_fk: cm.crate_fk,
            })
            .collect();

        Ok(crate_meta)
    }

    async fn update_last_updated(&self, id: i64, last_updated: &DateTime<Utc>) -> DbResult<()> {
        let krate = krate::Entity::find_by_id(id)
            .one(&self.db_con)
            .await?
            .ok_or(DbError::CrateNotFoundWithId(id))?;

        let date = last_updated.format(DB_DATE_FORMAT).to_string();

        let mut krate: krate::ActiveModel = krate.into();
        krate.last_updated = Set(date);
        krate.update(&self.db_con).await?;

        Ok(())
    }

    async fn search_in_crate_name(
        &self,
        contains: &str,
        cache: bool,
    ) -> DbResult<Vec<CrateOverview>> {
        let mut stmt_kellnr = Query::select()
            .expr_as(Expr::col(CrateIden::OriginalName), Alias::new("name"))
            .expr_as(Expr::col(CrateIden::MaxVersion), Alias::new("version"))
            .expr_as(Expr::col(CrateIden::LastUpdated), Alias::new("date"))
            .expr_as(
                Expr::col(CrateIden::TotalDownloads),
                Alias::new("total_downloads"),
            )
            .expr_as(Expr::col(CrateIden::Description), Alias::new("description"))
            .expr_as(
                Expr::col(CrateMetaIden::Documentation),
                Alias::new("documentation"),
            )
            .expr_as(Expr::cust("false"), Alias::new("is_cache"))
            .from(CrateMetaIden::Table)
            .inner_join(
                CrateIden::Table,
                Expr::col((CrateMetaIden::Table, CrateMetaIden::CrateFk))
                    .equals((CrateIden::Table, CrateIden::Id)),
            )
            .and_where(Expr::col((CrateIden::Table, CrateIden::Name)).like(format!("%{contains}%")))
            .and_where(
                Expr::col((CrateMetaIden::Table, CrateMetaIden::Version))
                    .equals((CrateIden::Table, CrateIden::MaxVersion)),
            )
            .to_owned();

        let stmt = if !cache {
            stmt_kellnr
                .order_by(CrateIden::OriginalName, Order::Asc)
                .to_owned()
        } else {
            stmt_kellnr
                .union(
                    UnionType::All,
                    Query::select()
                        .expr_as(Expr::col(CratesIoIden::OriginalName), Alias::new("name"))
                        .expr_as(Expr::col(CratesIoIden::MaxVersion), Alias::new("version"))
                        .expr_as(Expr::col(CratesIoIden::LastModified), Alias::new("date"))
                        .expr_as(
                            Expr::col(CratesIoIden::TotalDownloads),
                            Alias::new("total_downloads"),
                        )
                        .expr_as(
                            Expr::col(CratesIoIden::Description),
                            Alias::new("description"),
                        )
                        .expr_as(
                            Expr::col(CratesIoMetaIden::Documentation),
                            Alias::new("documentation"),
                        )
                        .expr_as(Expr::cust("true"), Alias::new("is_cache"))
                        .from(CratesIoMetaIden::Table)
                        .inner_join(
                            CratesIoIden::Table,
                            Expr::col((CratesIoMetaIden::Table, CratesIoMetaIden::CratesIoFk))
                                .equals((CratesIoIden::Table, CratesIoIden::Id)),
                        )
                        .and_where(
                            Expr::col((CratesIoMetaIden::Table, CratesIoMetaIden::Version))
                                .equals((CratesIoIden::Table, CratesIoIden::MaxVersion)),
                        )
                        .and_where(
                            Expr::col((CratesIoIden::Table, CrateIden::OriginalName))
                                .like(format!("%{contains}%")),
                        )
                        .to_owned(),
                )
                .order_by(Alias::new("name"), Order::Asc)
                .to_owned()
        };
        let builder = self.db_con.get_database_backend();
        let result = CrateOverview::find_by_statement(builder.build(&stmt))
            .all(&self.db_con)
            .await?;

        Ok(result)
    }

    async fn get_crate_overview_list(
        &self,
        limit: u64,
        offset: u64,
        cache: bool,
    ) -> DbResult<Vec<CrateOverview>> {
        let mut stmt_kellnr = Query::select()
            .expr_as(Expr::col(CrateIden::OriginalName), Alias::new("name"))
            .expr_as(Expr::col(CrateIden::MaxVersion), Alias::new("version"))
            .expr_as(Expr::col(CrateIden::LastUpdated), Alias::new("date"))
            .expr_as(
                Expr::col(CrateIden::TotalDownloads),
                Alias::new("total_downloads"),
            )
            .expr_as(Expr::col(CrateIden::Description), Alias::new("description"))
            .expr_as(
                Expr::col(CrateMetaIden::Documentation),
                Alias::new("documentation"),
            )
            .expr_as(Expr::cust("false"), Alias::new("is_cache"))
            .from(CrateMetaIden::Table)
            .inner_join(
                CrateIden::Table,
                Expr::col((CrateMetaIden::Table, CrateMetaIden::CrateFk))
                    .equals((CrateIden::Table, CrateIden::Id)),
            )
            .and_where(
                Expr::col((CrateMetaIden::Table, CrateMetaIden::Version))
                    .equals((CrateIden::Table, CrateIden::MaxVersion)),
            )
            .to_owned();

        let stmt = if !cache {
            stmt_kellnr
                .order_by(CrateIden::OriginalName, Order::Asc)
                .limit(limit)
                .offset(offset)
                .to_owned()
        } else {
            stmt_kellnr
                .union(
                    UnionType::All,
                    Query::select()
                        .expr_as(Expr::col(CratesIoIden::OriginalName), Alias::new("name"))
                        .expr_as(Expr::col(CratesIoIden::MaxVersion), Alias::new("version"))
                        .expr_as(Expr::col(CratesIoIden::LastModified), Alias::new("date"))
                        .expr_as(
                            Expr::col(CratesIoIden::TotalDownloads),
                            Alias::new("total_downloads"),
                        )
                        .expr_as(
                            Expr::col(CratesIoIden::Description),
                            Alias::new("description"),
                        )
                        .expr_as(
                            Expr::col(CratesIoMetaIden::Documentation),
                            Alias::new("documentation"),
                        )
                        .expr_as(Expr::cust("true"), Alias::new("is_cache"))
                        .from(CratesIoMetaIden::Table)
                        .inner_join(
                            CratesIoIden::Table,
                            Expr::col((CratesIoMetaIden::Table, CratesIoMetaIden::CratesIoFk))
                                .equals((CratesIoIden::Table, CratesIoIden::Id)),
                        )
                        .and_where(
                            Expr::col((CratesIoMetaIden::Table, CratesIoMetaIden::Version))
                                .equals((CratesIoIden::Table, CratesIoIden::MaxVersion)),
                        )
                        .to_owned(),
                )
                .order_by(Alias::new("name"), Order::Asc)
                .limit(limit)
                .offset(offset)
                .to_owned()
        };

        let builder = self.db_con.get_database_backend();
        let result = CrateOverview::find_by_statement(builder.build(&stmt))
            .all(&self.db_con)
            .await?;

        Ok(result)
    }

    async fn get_crate_data(&self, crate_name: &NormalizedName) -> DbResult<CrateData> {
        let krate = self.get_krate_model(crate_name).await?;

        let owners: Vec<String> = krate
            .find_related(owner::Entity)
            .find_also_related(user::Entity)
            .all(&self.db_con)
            .await?
            .into_iter()
            .filter_map(|(_, v)| v.map(|v| v.name))
            .collect();
        let categories: Vec<String> = krate
            .find_related(crate_category_to_crate::Entity)
            .find_also_related(crate_category::Entity)
            .all(&self.db_con)
            .await?
            .into_iter()
            .filter_map(|(_, v)| v.map(|v| v.category))
            .collect();
        let keywords: Vec<String> = krate
            .find_related(crate_keyword_to_crate::Entity)
            .find_also_related(crate_keyword::Entity)
            .all(&self.db_con)
            .await?
            .into_iter()
            .filter_map(|(_, v)| v.map(|v| v.keyword))
            .collect();
        let authors: Vec<String> = krate
            .find_related(crate_author_to_crate::Entity)
            .find_also_related(crate_author::Entity)
            .all(&self.db_con)
            .await?
            .into_iter()
            .filter_map(|(_, v)| v.map(|v| v.author))
            .collect();
        let crate_metas = krate
            .find_related(crate_meta::Entity)
            .all(&self.db_con)
            .await?;
        let crate_indices = krate
            .find_related(crate_index::Entity)
            .all(&self.db_con)
            .await?;

        let mut versions = Vec::new();
        for cm in crate_metas {
            let ci = crate_indices
                .iter()
                .find(|ci| ci.vers == cm.version)
                .ok_or(DbError::CrateIndexNotFound(
                    krate.name.clone(),
                    cm.version.clone(),
                ))?;
            let dependencies: Vec<CrateRegistryDep> = match ci.deps.clone() {
                Some(deps) => {
                    let ix = serde_json::from_value::<Vec<IndexDep>>(deps)
                        .map_err(|e| DbError::FailedToConvertToJson(e.to_string()))?;

                    let mut ft = Vec::new();
                    for dep in ix {
                        ft.push(CrateRegistryDep::from_index(
                            operations::get_desc_for_crate_dep(
                                &self.db_con,
                                &dep.name,
                                dep.registry.as_deref(),
                            )
                            .await?,
                            dep,
                        ));
                    }

                    ft
                }
                None => Vec::default(),
            };
            let features: BTreeMap<String, Vec<String>> = match ci.features.clone() {
                Some(features) => serde_json::from_value::<BTreeMap<String, Vec<String>>>(features)
                    .map_err(|e| DbError::FailedToConvertToJson(e.to_string()))?,
                None => BTreeMap::default(),
            };

            versions.push(CrateVersionData {
                version: cm.version,
                created: cm.created,
                downloads: cm.downloads,
                readme: cm.readme,
                license: cm.license,
                license_file: cm.license_file,
                documentation: cm.documentation,
                dependencies,
                checksum: ci.cksum.clone(),
                features,
                yanked: ci.yanked,
                links: ci.links.clone(),
                v: ci.v,
            });
        }
        versions.sort_by(|a, b| {
            Version::from_unchecked_str(&b.version).cmp(&Version::from_unchecked_str(&a.version))
        });

        let crate_data = CrateData {
            name: krate.original_name,
            owners,
            max_version: krate.max_version,
            total_downloads: krate.total_downloads,
            last_updated: krate.last_updated,
            homepage: krate.homepage,
            description: krate.description,
            repository: krate.repository,
            categories,
            keywords,
            authors,
            versions,
        };

        Ok(crate_data)
    }

    async fn add_empty_crate(&self, name: &str, created: &DateTime<Utc>) -> DbResult<i64> {
        let created = created.format(DB_DATE_FORMAT).to_string();
        let normalized_name = NormalizedName::from(
            OriginalName::try_from(name)
                .map_err(|_| DbError::InvalidCrateName(name.to_string()))?,
        );
        let krate = krate::ActiveModel {
            id: ActiveValue::default(),
            name: Set(normalized_name.to_string()),
            original_name: Set(name.to_string()),
            max_version: Set("0.0.0".to_string()),
            last_updated: Set(created.clone()),
            total_downloads: Set(0),
            homepage: Set(None),
            description: Set(None),
            repository: Set(None),
            e_tag: Set(String::new()), // Set to empty string, as it can be computed, when the crate index is inserted
            restricted_download: Set(false),
        };
        Ok(krate.insert(&self.db_con).await?.id)
    }

    async fn add_crate(
        &self,
        pub_metadata: &PublishMetadata,
        cksum: &str,
        created: &DateTime<Utc>,
        owner: &str,
    ) -> DbResult<i64> {
        let created = created.format(DB_DATE_FORMAT).to_string();
        let normalized_name = NormalizedName::from(
            OriginalName::try_from(&pub_metadata.name)
                .map_err(|_| DbError::InvalidCrateName(pub_metadata.name.clone()))?,
        );

        let existing = krate::Entity::find()
            .filter(krate::Column::Name.eq(pub_metadata.name.clone()))
            .one(&self.db_con)
            .await?;

        let txn = self.db_con.begin().await?;

        let crate_id = if let Some(krate) = existing {
            let krate_id = krate.id;
            let current_max_version = Version::try_from(&krate.max_version)
                .map_err(|_| DbError::InvalidVersion(krate.max_version.clone()))?;
            let max_version = current_max_version.max(
                Version::try_from(&pub_metadata.vers)
                    .map_err(|_| DbError::InvalidVersion(pub_metadata.vers.clone()))?,
            );

            let mut krate: krate::ActiveModel = krate.into();
            krate.last_updated = Set(created.clone());
            krate.max_version = Set(max_version.to_string());
            krate.homepage = Set(pub_metadata.homepage.clone());
            krate.description = Set(pub_metadata.description.clone());
            krate.repository = Set(pub_metadata.repository.clone());
            krate.e_tag = Set(String::new()); // Set to empty string, as it can be computed, when the crate index is inserted
            krate.update(&txn).await?;
            krate_id
        } else {
            let krate = krate::ActiveModel {
                id: ActiveValue::default(),
                name: Set(normalized_name.to_string()),
                original_name: Set(pub_metadata.name.clone()),
                max_version: Set(pub_metadata.vers.clone()),
                last_updated: Set(created.clone()),
                total_downloads: Set(0),
                homepage: Set(pub_metadata.homepage.clone()),
                description: Set(pub_metadata.description.clone()),
                repository: Set(pub_metadata.repository.clone()),
                e_tag: Set(String::new()), // Set to empty string, as it can be computed, when the crate index is inserted
                restricted_download: Set(false),
            };
            let krate = krate.insert(&txn).await?;
            krate.id
        };

        operations::add_owner_if_not_exists(&txn, owner, crate_id).await?;
        operations::add_crate_metadata(&txn, pub_metadata, &created, crate_id).await?;
        operations::add_crate_index(&txn, pub_metadata, cksum, crate_id).await?;
        operations::update_etag(&txn, &pub_metadata.name, crate_id).await?;
        operations::update_crate_categories(&txn, pub_metadata, crate_id).await?;
        operations::update_crate_keywords(&txn, pub_metadata, crate_id).await?;
        operations::update_crate_authors(&txn, pub_metadata, crate_id).await?;

        txn.commit().await?;
        Ok(crate_id)
    }

    async fn update_docs_link(
        &self,
        crate_name: &NormalizedName,
        version: &Version,
        docs_link: &str,
    ) -> DbResult<()> {
        let (cm, _c) = crate_meta::Entity::find()
            .find_also_related(krate::Entity)
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name.to_string()))
                    .add(crate_meta::Column::Version.eq(version.to_string())),
            )
            .one(&self.db_con)
            .await?
            .ok_or(DbError::CrateNotFound(crate_name.to_string()))?;

        let mut cm: crate_meta::ActiveModel = cm.into();
        cm.documentation = Set(Some(docs_link.to_string()));
        cm.update(&self.db_con).await?;
        Ok(())
    }

    async fn add_crate_metadata(
        &self,
        pub_metadata: &PublishMetadata,
        created: &str,
        crate_id: i64,
    ) -> DbResult<()> {
        operations::add_crate_metadata(&self.db_con, pub_metadata, created, crate_id).await
    }

    async fn get_prefetch_data(&self, crate_name: &str) -> DbResult<Prefetch> {
        let krate = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name))
            .find_with_related(crate_index::Entity)
            .all(&self.db_con)
            .await?;

        // Exactly one crate must be returned.
        if krate.len() != 1 {
            return Err(DbError::CrateNotFound(crate_name.to_string()));
        }

        let (krate, crate_indices) = krate[0].to_owned();
        let index_metadata =
            operations::crate_index_model_to_index_metadata(crate_name, crate_indices)?;
        let data = operations::index_metadata_to_bytes(&index_metadata)?;

        Ok(Prefetch {
            data,
            etag: krate.e_tag.clone(),
            last_modified: krate.last_updated,
        })
    }

    async fn is_cratesio_cache_up_to_date(
        &self,
        crate_name: &NormalizedName,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> DbResult<PrefetchState> {
        let Some(krate) = cratesio_crate::Entity::find()
            .filter(cratesio_crate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
        else {
            return Ok(PrefetchState::NotFound);
        };

        let needs_update = match (etag, last_modified) {
            (Some(etag), Some(last_modified)) => {
                krate.e_tag != etag || krate.last_modified != last_modified
            }
            (Some(etag), None) => krate.e_tag != etag,
            (None, Some(last_modified)) => krate.last_modified != last_modified,
            (None, None) => true,
        };

        if !needs_update {
            Ok(PrefetchState::UpToDate)
        } else {
            let crate_indices = krate
                .find_related(cratesio_index::Entity)
                .all(&self.db_con)
                .await?;
            let index_metadata =
                operations::cratesio_index_model_to_index_metadata(crate_name, crate_indices)?;
            let data = operations::index_metadata_to_bytes(&index_metadata)?;

            Ok(PrefetchState::NeedsUpdate(Prefetch {
                data,
                etag: krate.e_tag.clone(),
                last_modified: krate.last_modified,
            }))
        }
    }

    async fn add_cratesio_prefetch_data(
        &self,
        crate_name: &OriginalName,
        etag: &str,
        last_modified: &str,
        description: Option<String>,
        indices: &[IndexMetadata],
    ) -> DbResult<Prefetch> {
        let normalized_name = crate_name.to_normalized();

        let max_version = indices
            .iter()
            .map(|i| Version::from_unchecked_str(&i.vers))
            .max()
            .ok_or(DbError::FailedToGetMaxVersionByName(crate_name.to_string()))?;

        let krate = cratesio_crate::Entity::find()
            .filter(cratesio_crate::Column::Name.eq(normalized_name.to_string()))
            .one(&self.db_con)
            .await?;

        let krate = if let Some(krate) = krate {
            let mut krate: cratesio_crate::ActiveModel = krate.into();
            krate.e_tag = Set(etag.to_string());
            krate.last_modified = Set(last_modified.to_string());
            krate.max_version = Set(max_version.to_string());
            krate.update(&self.db_con).await?
        } else {
            let krate = cratesio_crate::ActiveModel {
                id: ActiveValue::default(),
                name: Set(normalized_name.to_string()),
                original_name: Set(crate_name.to_string()),
                description: Set(description),
                e_tag: Set(etag.to_string()),
                last_modified: Set(last_modified.to_string()),
                total_downloads: Set(0),
                max_version: Set(max_version.to_string()),
            };
            krate.insert(&self.db_con).await?
        };

        let current_indices = cratesio_index::Entity::find()
            .filter(cratesio_index::Column::CratesIoFk.eq(krate.id))
            .all(&self.db_con)
            .await?;

        for index in indices {
            // Check if the version was yanked or un-yanked and update if so.
            if let Some(current_index) = current_indices.iter().find(|ci| index.vers == ci.vers) {
                if index.yanked != current_index.yanked {
                    let mut ci: cratesio_index::ActiveModel = current_index.to_owned().into();
                    ci.yanked = Set(index.yanked);
                    ci.update(&self.db_con).await?;
                }
            } else {
                let deps = if index.deps.is_empty() {
                    None
                } else {
                    let deps = serde_json::to_value(&index.deps)
                        .map_err(|e| DbError::FailedToConvertToJson(e.to_string()))?;
                    Some(deps)
                };

                let features = serde_json::to_value(&index.features)
                    .map_err(|e| DbError::FailedToConvertToJson(e.to_string()))?;

                let features2 = serde_json::to_value(&index.features2)
                    .map_err(|e| DbError::FailedToConvertToJson(e.to_string()))?;

                let new_index = cratesio_index::ActiveModel {
                    id: ActiveValue::default(),
                    name: Set(index.name.clone()),
                    vers: Set(index.vers.clone()),
                    deps: Set(deps),
                    cksum: Set(index.cksum.clone()),
                    features: Set(Some(features)),
                    features2: Set(Some(features2)),
                    yanked: Set(index.yanked),
                    links: Set(index.links.clone()),
                    pubtime: Set(index.pubtime.map(|dt| dt.naive_utc())),
                    v: Set(index.v.unwrap_or(1) as i32),
                    crates_io_fk: Set(krate.id),
                };

                new_index.insert(&self.db_con).await?;

                // Add the meta data for the crate version.
                let meta = cratesio_meta::ActiveModel {
                    id: ActiveValue::default(),
                    version: Set(index.vers.clone()),
                    downloads: Set(0),
                    crates_io_fk: Set(krate.id),
                    documentation: Set(Some(format!(
                        "https://docs.rs/{normalized_name}/{}",
                        index.vers,
                    ))),
                };

                meta.insert(&self.db_con).await?;
            }
        }

        Ok(Prefetch {
            data: operations::index_metadata_to_bytes(indices)?,
            etag: etag.to_string(),
            last_modified: last_modified.to_string(),
        })
    }

    async fn get_cratesio_index_update_list(&self) -> DbResult<Vec<CratesioPrefetchMsg>> {
        let crates = cratesio_crate::Entity::find().all(&self.db_con).await?;
        let msgs = crates
            .into_iter()
            .map(|krate| {
                CratesioPrefetchMsg::Update(UpdateData {
                    name: OriginalName::from_unchecked(krate.original_name),
                    etag: Some(krate.e_tag),
                    last_modified: Some(krate.last_modified),
                })
            })
            .collect();
        Ok(msgs)
    }

    async fn unyank_crate(&self, crate_name: &NormalizedName, version: &Version) -> DbResult<()> {
        let mut ci: crate_index::ActiveModel = self
            .get_crate_index_model(crate_name, version)
            .await?
            .into();
        ci.yanked = Set(false);
        ci.save(&self.db_con).await?;
        Ok(())
    }

    async fn yank_crate(&self, crate_name: &NormalizedName, version: &Version) -> DbResult<()> {
        let mut ci: crate_index::ActiveModel = self
            .get_crate_index_model(crate_name, version)
            .await?
            .into();
        ci.yanked = Set(true);
        ci.save(&self.db_con).await?;
        Ok(())
    }

    async fn register_webhook(&self, webhook: Webhook) -> DbResult<String> {
        let w = webhook::ActiveModel {
            event: Set(Into::<&str>::into(webhook.event).to_string()),
            callback_url: Set(webhook.callback_url),
            name: Set(webhook.name),
            ..Default::default()
        };

        let w: webhook::Model = w.insert(&self.db_con).await?;
        Ok(w.id.to_string())
    }
    async fn delete_webhook(&self, id: &str) -> DbResult<()> {
        let w = webhook::Entity::find()
            .filter(webhook::Column::Id.eq(
                TryInto::<Uuid>::try_into(id).map_err(|_| DbError::InvalidId(id.to_string()))?,
            ))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::WebhookNotFound)?;

        w.delete(&self.db_con).await?;
        Ok(())
    }
    async fn get_webhook(&self, id: &str) -> DbResult<Webhook> {
        let w = webhook::Entity::find()
            .filter(webhook::Column::Id.eq(
                TryInto::<Uuid>::try_into(id).map_err(|_| DbError::InvalidId(id.to_string()))?,
            ))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::WebhookNotFound)?;

        Ok(Webhook {
            id: Some(w.id.into()),
            name: w.name,
            event: w
                .event
                .as_str()
                .try_into()
                .map_err(|_| DbError::InvalidWebhookEvent(w.event))?,
            callback_url: w.callback_url,
        })
    }
    async fn get_all_webhooks(&self) -> DbResult<Vec<Webhook>> {
        let w = webhook::Entity::find().all(&self.db_con).await?;

        Ok(w.into_iter()
            .filter_map(|w| {
                Some(Webhook {
                    id: Some(w.id.into()),
                    name: w.name,
                    // Entries with invalid events would get skipped
                    event: w.event.as_str().try_into().ok()?,
                    callback_url: w.callback_url,
                })
            })
            .collect())
    }
    async fn add_webhook_queue(
        &self,
        event: WebhookEvent,
        payload: serde_json::Value,
    ) -> DbResult<()> {
        let w = webhook::Entity::find()
            .filter(webhook::Column::Event.eq(Into::<&str>::into(event)))
            .all(&self.db_con)
            .await?;

        if w.is_empty() {
            return Ok(());
        }

        let now = Utc::now();

        let entries = w.iter().map(|w| webhook_queue::ActiveModel {
            webhook_fk: Set(w.id),
            payload: Set(payload.clone()),
            next_attempt: Set(now.into()),
            last_attempt: Set(None),
            ..Default::default()
        });

        webhook_queue::Entity::insert_many(entries)
            .exec(&self.db_con)
            .await?;
        Ok(())
    }
    async fn get_pending_webhook_queue_entries(
        &self,
        timestamp: DateTime<Utc>,
    ) -> DbResult<Vec<WebhookQueue>> {
        let w = webhook_queue::Entity::find()
            .find_with_related(webhook::Entity)
            .filter(webhook_queue::Column::NextAttempt.lte(timestamp))
            .all(&self.db_con)
            .await?;

        Ok(w.iter()
            .filter_map(|w| {
                Some(WebhookQueue {
                    id: Into::<String>::into(w.0.id),
                    callback_url: w.1.first()?.callback_url.clone(),
                    payload: w.0.payload.clone(),
                    last_attempt: w.0.last_attempt.map(Into::into),
                    next_attempt: w.0.next_attempt.into(),
                })
            })
            .collect())
    }
    async fn update_webhook_queue(
        &self,
        id: &str,
        last_attempt: DateTime<Utc>,
        next_attempt: DateTime<Utc>,
    ) -> DbResult<()> {
        let w = webhook_queue::Entity::find()
            .filter(webhook_queue::Column::Id.eq(
                TryInto::<Uuid>::try_into(id).map_err(|_| DbError::InvalidId(id.to_string()))?,
            ))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::WebhookNotFound)?;

        let mut w: webhook_queue::ActiveModel = w.into();
        w.last_attempt = Set(Some(last_attempt.into()));
        w.next_attempt = Set(next_attempt.into());
        w.update(&self.db_con).await?;
        Ok(())
    }
    async fn delete_webhook_queue(&self, id: &str) -> DbResult<()> {
        let w = webhook_queue::Entity::find()
            .filter(webhook_queue::Column::Id.eq(
                TryInto::<Uuid>::try_into(id).map_err(|_| DbError::InvalidId(id.to_string()))?,
            ))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::WebhookNotFound)?;

        w.delete(&self.db_con).await?;
        Ok(())
    }

    // OAuth2 identity methods

    async fn get_user_by_oauth2_identity(
        &self,
        issuer: &str,
        subject: &str,
    ) -> DbResult<Option<User>> {
        let identity = oauth2_identity::Entity::find()
            .filter(oauth2_identity::Column::ProviderIssuer.eq(issuer))
            .filter(oauth2_identity::Column::Subject.eq(subject))
            .one(&self.db_con)
            .await?;

        if let Some(identity) = identity {
            let u = user::Entity::find_by_id(identity.user_fk)
                .one(&self.db_con)
                .await?
                .ok_or_else(|| DbError::UserNotFound(format!("user_id={}", identity.user_fk)))?;

            Ok(Some(User::from(u)))
        } else {
            Ok(None)
        }
    }

    async fn create_oauth2_user(
        &self,
        username: &str,
        issuer: &str,
        subject: &str,
        email: Option<String>,
        is_admin: bool,
        is_read_only: bool,
    ) -> DbResult<User> {
        // Use a transaction to ensure atomicity
        let txn = self.db_con.begin().await?;

        // Generate a random password and salt for OAuth2 users
        // They won't use password auth, but we need valid values
        let salt = generate_salt();
        let random_pwd = Uuid::new_v4().to_string();
        let hashed_pwd = hash_pwd(&random_pwd, &salt);
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();

        // Create the user
        let new_user = user::ActiveModel {
            name: Set(username.to_string()),
            pwd: Set(hashed_pwd),
            salt: Set(salt),
            is_admin: Set(is_admin),
            is_read_only: Set(is_read_only),
            created: Set(created.clone()),
            ..Default::default()
        };

        let res = user::Entity::insert(new_user).exec(&txn).await?;
        let user_id = res.last_insert_id;

        // Link the OAuth2 identity
        let identity = oauth2_identity::ActiveModel {
            user_fk: Set(user_id),
            provider_issuer: Set(issuer.to_string()),
            subject: Set(subject.to_string()),
            email: Set(email),
            created: Set(created.clone()),
            ..Default::default()
        };

        oauth2_identity::Entity::insert(identity).exec(&txn).await?;

        txn.commit().await?;

        Ok(User {
            id: user_id as i32,
            name: username.to_string(),
            pwd: String::new(),  // Don't expose password hash
            salt: String::new(), // Don't expose salt
            is_admin,
            is_read_only,
            created,
        })
    }

    async fn link_oauth2_identity(
        &self,
        user_id: i64,
        issuer: &str,
        subject: &str,
        email: Option<String>,
    ) -> DbResult<()> {
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();
        let identity = oauth2_identity::ActiveModel {
            user_fk: Set(user_id),
            provider_issuer: Set(issuer.to_string()),
            subject: Set(subject.to_string()),
            email: Set(email),
            created: Set(created),
            ..Default::default()
        };

        oauth2_identity::Entity::insert(identity)
            .exec(&self.db_con)
            .await?;

        Ok(())
    }

    // OAuth2 state methods (CSRF/PKCE during auth flow)

    async fn store_oauth2_state(
        &self,
        state: &str,
        pkce_verifier: &str,
        nonce: &str,
    ) -> DbResult<()> {
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();
        let s = oauth2_state::ActiveModel {
            state: Set(state.to_string()),
            pkce_verifier: Set(pkce_verifier.to_string()),
            nonce: Set(nonce.to_string()),
            created: Set(created),
            ..Default::default()
        };

        oauth2_state::Entity::insert(s).exec(&self.db_con).await?;
        Ok(())
    }

    async fn get_and_delete_oauth2_state(&self, state: &str) -> DbResult<Option<OAuth2StateData>> {
        let s = oauth2_state::Entity::find()
            .filter(oauth2_state::Column::State.eq(state))
            .one(&self.db_con)
            .await?;

        if let Some(s) = s {
            let data = OAuth2StateData {
                state: s.state.clone(),
                pkce_verifier: s.pkce_verifier.clone(),
                nonce: s.nonce.clone(),
            };

            // Delete after retrieving (single use)
            s.delete(&self.db_con).await?;

            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    async fn cleanup_expired_oauth2_states(&self) -> DbResult<u64> {
        // States older than 10 minutes are expired
        let expiry = Utc::now() - chrono::Duration::minutes(10);
        let expiry_str = expiry.format(DB_DATE_FORMAT).to_string();

        let result = oauth2_state::Entity::delete_many()
            .filter(oauth2_state::Column::Created.lt(expiry_str))
            .exec(&self.db_con)
            .await?;

        Ok(result.rows_affected)
    }

    async fn is_username_available(&self, username: &str) -> DbResult<bool> {
        let existing = user::Entity::find()
            .filter(user::Column::Name.eq(username))
            .one(&self.db_con)
            .await?;

        Ok(existing.is_none())
    }

    // Toolchain distribution methods

    async fn add_toolchain(
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

    async fn add_toolchain_target(
        &self,
        toolchain_id: i64,
        target: &str,
        storage_path: &str,
        hash: &str,
        size: i64,
    ) -> DbResult<()> {
        let model = toolchain_target::ActiveModel {
            id: ActiveValue::NotSet,
            toolchain_fk: Set(toolchain_id),
            target: Set(target.to_string()),
            storage_path: Set(storage_path.to_string()),
            hash: Set(hash.to_string()),
            size: Set(size),
        };

        model.insert(&self.db_con).await?;
        Ok(())
    }

    async fn get_toolchain_by_channel(
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

    async fn get_toolchain_by_version(
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

    async fn list_toolchains(&self) -> DbResult<Vec<ToolchainWithTargets>> {
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

    async fn delete_toolchain(&self, name: &str, version: &str) -> DbResult<()> {
        toolchain::Entity::delete_many()
            .filter(toolchain::Column::Name.eq(name))
            .filter(toolchain::Column::Version.eq(version))
            .exec(&self.db_con)
            .await?;

        Ok(())
    }

    async fn delete_toolchain_target(
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

    async fn set_channel(&self, channel: &str, name: &str, version: &str) -> DbResult<()> {
        // First, clear the channel from any other toolchain that has it
        let toolchains_with_channel = toolchain::Entity::find()
            .filter(toolchain::Column::Channel.eq(channel))
            .all(&self.db_con)
            .await?;

        for tc in toolchains_with_channel {
            let mut model: toolchain::ActiveModel = tc.into();
            model.channel = Set(None);
            model.update(&self.db_con).await?;
        }

        // Then set the channel on the target toolchain
        if let Some(tc) = self.get_toolchain_by_name_version(name, version).await? {
            let mut model: toolchain::ActiveModel = tc.into();
            model.channel = Set(Some(channel.to_string()));
            model.update(&self.db_con).await?;
        }

        Ok(())
    }

    async fn get_channels(&self) -> DbResult<Vec<ChannelInfo>> {
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
