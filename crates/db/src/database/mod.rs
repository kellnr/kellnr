mod acl;
mod auth_tokens;
mod crates;
mod cratesio_cache;
mod docs;
mod groups;
mod oauth2;
mod operations;
mod search;
mod sessions;
mod stats;
#[cfg(feature = "test-support")]
pub mod test_utils;
mod toolchains;
mod users;
mod webhooks;

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use kellnr_common::account_name::{GroupName, UserName};
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::crate_data::CrateData;
use kellnr_common::crate_dependents::CrateDependents;
use kellnr_common::crate_overview::CrateOverview;
use kellnr_common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::original_name::OriginalName;
use kellnr_common::plain_token::PlainToken;
use kellnr_common::prefetch::Prefetch;
use kellnr_common::publish_metadata::PublishMetadata;
use kellnr_common::version::Version;
use kellnr_common::webhook::{Webhook, WebhookEvent, WebhookQueue};
use kellnr_entity::{group, krate, user};
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::query::TransactionTrait;
use sea_orm::sea_query::{Alias, Expr, Func, Iden, Query};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, ExprTrait,
    FromQueryResult, ModelTrait, QueryFilter, Statement,
};
use tracing::warn;

use crate::error::DbError;
use crate::provider::{
    ChannelInfo, CrateSort, CrateTagFilter, CrateVisibility, CratesioVersionMeta, DbResult,
    OAuth2StateData, PrefetchState, SessionInfo, ToolchainWithTargets,
};
use crate::tables::init_database;
use crate::{
    AuthToken, AuthTokenId, ConString, CrateId, CrateMeta, DbProvider, DocQueueEntry, Group,
    ToolchainTargetStatus, User, UserFlags,
};

const DB_DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

use kellnr_settings::SessionAge;

/// Resolve a configured session age into the internal expiry duration,
/// [`None`] if sessions never expire.
fn session_age_to_duration(age: SessionAge) -> Option<chrono::Duration> {
    // The expiry is at most `MAX_SESSION_AGE`, which fits comfortably in a
    // `chrono::Duration`, so the conversion cannot fail.
    age.expiry()
        .map(|d| chrono::Duration::from_std(d).expect("bounded session age fits chrono::Duration"))
}

#[derive(Debug, PartialEq, FromQueryResult)]
struct CountResult {
    count: Option<i64>,
}

pub struct Database {
    db_con: DatabaseConnection,
    /// Maximum lifetime of a session before it is treated as expired, or
    /// `None` if sessions never expire (configured age of zero).
    session_age: Option<chrono::Duration>,
}

impl Database {
    pub async fn new(con: &ConString, max_con: u32) -> Result<Self, DbError> {
        let db_con = init_database(con, max_con)
            .await
            .map_err(|e| DbError::InitializationError(e.to_string()))?;

        if operations::no_user_exists(&db_con).await? {
            operations::insert_admin_credentials(&db_con, con).await?;
        }

        Ok(Self {
            db_con,
            session_age: session_age_to_duration(con.session_age()),
        })
    }

    /// Runs `f` in a transaction that is committed if `f` succeeds. If `f`
    /// fails, the transaction is rolled back right away. A dropped transaction
    /// is only rolled back later, so on `SQLite` the next write could still
    /// find the database locked.
    async fn in_transaction<T>(
        &self,
        f: impl AsyncFnOnce(&DatabaseTransaction) -> DbResult<T>,
    ) -> DbResult<T> {
        let txn = self.db_con.begin().await?;
        match f(&txn).await {
            Ok(value) => {
                txn.commit().await?;
                Ok(value)
            }
            Err(e) => {
                if let Err(rollback) = txn.rollback().await {
                    warn!("Failed to roll back transaction: {rollback}");
                }
                Err(e)
            }
        }
    }

    /// Looks up a user by name; returns the entity model or [`DbError::UserNotFound`].
    async fn get_user_model(&self, name: &UserName) -> DbResult<user::Model> {
        user::Entity::find()
            .filter(user::Column::Name.eq(name))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::UserNotFound(name.to_string()))
    }

    /// Looks up a krate by canonical name; returns the entity model or [`DbError::CrateNotFound`].
    async fn get_krate_model(&self, crate_name: &CanonicalName) -> DbResult<krate::Model> {
        krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))
    }

    /// Looks up a group by name; returns the entity model or [`DbError::GroupNotFound`].
    async fn get_group_model(&self, name: &GroupName) -> DbResult<group::Model> {
        group::Entity::find()
            .filter(group::Column::Name.eq(name))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::GroupNotFound(name.to_string()))
    }

    /// Executes a count query `SELECT COUNT(id_column) FROM table` and returns the count as u64.
    async fn count<T>(&self, table: T, id_column: T, error: DbError) -> DbResult<u64>
    where
        T: Iden + Copy,
    {
        let statement = self.db_con.get_database_backend().build(
            Query::select()
                .expr_as(Expr::col((table, id_column)).count(), Alias::new("count"))
                .from(table),
        );
        self.count_by_statement(statement, error).await
    }

    /// Executes `SELECT SUM(column) FROM table` and returns the sum as u64, 0 for an
    /// empty table. Cast, as PostgreSQL sums a bigint column to a numeric.
    async fn sum<T>(&self, table: T, column: T, error: DbError) -> DbResult<u64>
    where
        T: Iden + Copy + 'static,
    {
        let sum = Func::coalesce([Expr::col((table, column)).sum(), Expr::val(0)]);
        let statement = self.db_con.get_database_backend().build(
            Query::select()
                .expr_as(
                    Func::cast_as(sum, Alias::new("BIGINT")),
                    Alias::new("count"),
                )
                .from(table),
        );
        self.count_by_statement(statement, error).await
    }

    /// Runs a statement that selects a single `count` column and returns it as u64.
    async fn count_by_statement(&self, statement: Statement, error: DbError) -> DbResult<u64> {
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

    async fn delete_or_not_found<M, A>(&self, model: Option<M>, err: DbError) -> DbResult<()>
    where
        M: ModelTrait + sea_orm::IntoActiveModel<A>,
        A: ActiveModelTrait<Entity = M::Entity> + sea_orm::ActiveModelBehavior + Send,
    {
        model.ok_or(err)?.delete(&self.db_con).await?;
        Ok(())
    }
}

// The methods are implemented per domain in the submodules. Their inherent
// methods take precedence over the trait methods of the same name.
#[async_trait]
impl DbProvider for Database {
    async fn get_total_unique_cached_crates(&self) -> DbResult<u64> {
        Database::get_total_unique_cached_crates(self).await
    }

    async fn get_total_cached_crate_versions(&self) -> DbResult<u64> {
        Database::get_total_cached_crate_versions(self).await
    }

    async fn get_total_cached_downloads(&self) -> DbResult<u64> {
        Database::get_total_cached_downloads(self).await
    }

    async fn authenticate_user(&self, name: &UserName, pwd: &str) -> DbResult<User> {
        Database::authenticate_user(self, name, pwd).await
    }

    async fn increase_download_counter(
        &self,
        crate_name: &CanonicalName,
        crate_version: &Version,
    ) -> DbResult<()> {
        Database::increase_download_counter(self, crate_name, crate_version).await
    }

    async fn increase_cached_download_counter(
        &self,
        crate_name: &CanonicalName,
        crate_version: &Version,
    ) -> DbResult<()> {
        Database::increase_cached_download_counter(self, crate_name, crate_version).await
    }

    async fn increase_download_counter_by(
        &self,
        crate_name: &CanonicalName,
        crate_version: &Version,
        count: u64,
    ) -> DbResult<()> {
        Database::increase_download_counter_by(self, crate_name, crate_version, count).await
    }

    async fn increase_cached_download_counter_by(
        &self,
        crate_name: &CanonicalName,
        crate_version: &Version,
        count: u64,
    ) -> DbResult<()> {
        Database::increase_cached_download_counter_by(self, crate_name, crate_version, count).await
    }

    async fn get_last_updated_crate(
        &self,
        visibility: &CrateVisibility,
    ) -> DbResult<Option<(OriginalName, Version)>> {
        Database::get_last_updated_crate(self, visibility).await
    }

    async fn validate_session(&self, session_token: &PlainToken) -> DbResult<SessionInfo> {
        Database::validate_session(self, session_token).await
    }

    async fn delete_expired_sessions(&self) -> DbResult<u64> {
        Database::delete_expired_sessions(self).await
    }

    async fn add_session_token(&self, name: &UserName, session_token: &PlainToken) -> DbResult<()> {
        Database::add_session_token(self, name, session_token).await
    }

    async fn delete_user_sessions(&self, user_name: &UserName) -> DbResult<u64> {
        Database::delete_user_sessions(self, user_name).await
    }

    async fn delete_other_user_sessions(
        &self,
        user_name: &UserName,
        keep_session_token: &PlainToken,
    ) -> DbResult<u64> {
        Database::delete_other_user_sessions(self, user_name, keep_session_token).await
    }

    async fn add_crate_user(&self, crate_name: &CanonicalName, user: &UserName) -> DbResult<()> {
        Database::add_crate_user(self, crate_name, user).await
    }

    async fn add_crate_group(&self, crate_name: &CanonicalName, group: &GroupName) -> DbResult<()> {
        Database::add_crate_group(self, crate_name, group).await
    }

    async fn add_group_user(&self, group_name: &GroupName, user: &UserName) -> DbResult<()> {
        Database::add_group_user(self, group_name, user).await
    }

    async fn add_owner(&self, crate_name: &CanonicalName, owner: &UserName) -> DbResult<()> {
        Database::add_owner(self, crate_name, owner).await
    }

    async fn is_download_restricted(&self, crate_name: &CanonicalName) -> DbResult<bool> {
        Database::is_download_restricted(self, crate_name).await
    }

    async fn change_download_restricted(
        &self,
        crate_name: &CanonicalName,
        restricted: bool,
    ) -> DbResult<()> {
        Database::change_download_restricted(self, crate_name, restricted).await
    }

    async fn is_crate_user(&self, crate_name: &CanonicalName, user: &UserName) -> DbResult<bool> {
        Database::is_crate_user(self, crate_name, user).await
    }

    async fn is_crate_group(
        &self,
        crate_name: &CanonicalName,
        group: &GroupName,
    ) -> DbResult<bool> {
        Database::is_crate_group(self, crate_name, group).await
    }

    async fn is_crate_group_user(
        &self,
        crate_name: &CanonicalName,
        user: &UserName,
    ) -> DbResult<bool> {
        Database::is_crate_group_user(self, crate_name, user).await
    }

    async fn is_group_user(&self, group_name: &GroupName, user: &UserName) -> DbResult<bool> {
        Database::is_group_user(self, group_name, user).await
    }

    async fn is_owner(&self, crate_name: &CanonicalName, user: &UserName) -> DbResult<bool> {
        Database::is_owner(self, crate_name, user).await
    }

    async fn get_crate_id(&self, crate_name: &CanonicalName) -> DbResult<Option<CrateId>> {
        Database::get_crate_id(self, crate_name).await
    }

    async fn get_crate_owners(&self, crate_name: &CanonicalName) -> DbResult<Vec<User>> {
        Database::get_crate_owners(self, crate_name).await
    }

    async fn get_crate_users(&self, crate_name: &CanonicalName) -> DbResult<Vec<User>> {
        Database::get_crate_users(self, crate_name).await
    }

    async fn get_crate_groups(&self, crate_name: &CanonicalName) -> DbResult<Vec<Group>> {
        Database::get_crate_groups(self, crate_name).await
    }

    async fn get_group_users(&self, group_name: &GroupName) -> DbResult<Vec<User>> {
        Database::get_group_users(self, group_name).await
    }

    async fn get_crate_versions(&self, crate_name: &CanonicalName) -> DbResult<Vec<Version>> {
        Database::get_crate_versions(self, crate_name).await
    }

    async fn delete_session_token(&self, session_token: &PlainToken) -> DbResult<()> {
        Database::delete_session_token(self, session_token).await
    }

    async fn delete_user(&self, user_name: &UserName) -> DbResult<()> {
        Database::delete_user(self, user_name).await
    }

    async fn delete_group(&self, group_name: &GroupName) -> DbResult<()> {
        Database::delete_group(self, group_name).await
    }

    async fn change_pwd(&self, user_name: &UserName, new_pwd: &str) -> DbResult<()> {
        Database::change_pwd(self, user_name, new_pwd).await
    }

    async fn change_read_only_state(&self, user_name: &UserName, state: bool) -> DbResult<()> {
        Database::change_read_only_state(self, user_name, state).await
    }

    async fn change_admin_state(&self, user_name: &UserName, state: bool) -> DbResult<()> {
        Database::change_admin_state(self, user_name, state).await
    }

    async fn crate_version_exists(&self, crate_id: CrateId, version: &Version) -> DbResult<bool> {
        Database::crate_version_exists(self, crate_id, version).await
    }

    async fn get_max_version_from_id(&self, crate_id: CrateId) -> DbResult<Version> {
        Database::get_max_version_from_id(self, crate_id).await
    }

    async fn get_max_version_from_name(&self, crate_name: &CanonicalName) -> DbResult<Version> {
        Database::get_max_version_from_name(self, crate_name).await
    }

    async fn add_auth_token(
        &self,
        name: &str,
        token: &PlainToken,
        user: &UserName,
    ) -> DbResult<()> {
        Database::add_auth_token(self, name, token, user).await
    }

    async fn get_user_from_token(&self, token: &PlainToken) -> DbResult<User> {
        Database::get_user_from_token(self, token).await
    }

    async fn get_user(&self, name: &UserName) -> DbResult<User> {
        Database::get_user(self, name).await
    }

    async fn get_auth_token_name(&self, token: &PlainToken) -> DbResult<Option<String>> {
        Database::get_auth_token_name(self, token).await
    }

    async fn get_auth_tokens(&self, user_name: &UserName) -> DbResult<Vec<AuthToken>> {
        Database::get_auth_tokens(self, user_name).await
    }

    async fn delete_auth_token(&self, user_name: &UserName, id: AuthTokenId) -> DbResult<()> {
        Database::delete_auth_token(self, user_name, id).await
    }

    async fn delete_owners(
        &self,
        crate_name: &CanonicalName,
        owners: &[UserName],
        allow_ownerless: bool,
    ) -> DbResult<()> {
        Database::delete_owners(self, crate_name, owners, allow_ownerless).await
    }

    async fn delete_crate_user(&self, crate_name: &CanonicalName, user: &UserName) -> DbResult<()> {
        Database::delete_crate_user(self, crate_name, user).await
    }

    async fn delete_crate_group(
        &self,
        crate_name: &CanonicalName,
        group: &GroupName,
    ) -> DbResult<()> {
        Database::delete_crate_group(self, crate_name, group).await
    }

    async fn delete_group_user(&self, group_name: &GroupName, user: &UserName) -> DbResult<()> {
        Database::delete_group_user(self, group_name, user).await
    }

    async fn add_user(&self, name: &UserName, pwd: &str, flags: UserFlags) -> DbResult<()> {
        Database::add_user(self, name, pwd, flags).await
    }

    async fn add_group(&self, name: &GroupName) -> DbResult<()> {
        Database::add_group(self, name).await
    }

    async fn get_users(&self) -> DbResult<Vec<User>> {
        Database::get_users(self).await
    }

    async fn get_groups(&self) -> DbResult<Vec<Group>> {
        Database::get_groups(self).await
    }

    async fn get_total_unique_crates(&self) -> DbResult<u64> {
        Database::get_total_unique_crates(self).await
    }

    async fn get_total_crate_versions(&self) -> DbResult<u64> {
        Database::get_total_crate_versions(self).await
    }

    async fn get_total_downloads(&self) -> DbResult<u64> {
        Database::get_total_downloads(self).await
    }

    async fn get_top_crates_downloads(
        &self,
        top: u64,
        visibility: &CrateVisibility,
    ) -> DbResult<Vec<(String, u64)>> {
        Database::get_top_crates_downloads(self, top, visibility).await
    }

    async fn add_doc_queue(
        &self,
        krate: &OriginalName,
        version: &Version,
        path: &Path,
    ) -> DbResult<()> {
        Database::add_doc_queue(self, krate, version, path).await
    }

    async fn delete_doc_queue(&self, id: i64) -> DbResult<()> {
        Database::delete_doc_queue(self, id).await
    }

    async fn get_doc_queue(&self) -> DbResult<Vec<DocQueueEntry>> {
        Database::get_doc_queue(self).await
    }

    async fn delete_crate(&self, krate: &CanonicalName, version: &Version) -> DbResult<()> {
        Database::delete_crate(self, krate, version).await
    }

    async fn get_crate_meta_list(&self, crate_name: &CanonicalName) -> DbResult<Vec<CrateMeta>> {
        Database::get_crate_meta_list(self, crate_name).await
    }

    async fn search_in_crate_name_and_description(
        &self,
        contains: &str,
        limit: u64,
        offset: u64,
        cache: bool,
        visibility: &CrateVisibility,
    ) -> DbResult<Vec<CrateOverview>> {
        Database::search_in_crate_name_and_description(
            self, contains, limit, offset, cache, visibility,
        )
        .await
    }

    async fn count_by_crate_name_and_description(
        &self,
        contains: &str,
        cache: bool,
        visibility: &CrateVisibility,
    ) -> DbResult<u64> {
        Database::count_by_crate_name_and_description(self, contains, cache, visibility).await
    }

    async fn get_crate_overview_list(
        &self,
        limit: u64,
        offset: u64,
        cache: bool,
        visibility: &CrateVisibility,
        sort: CrateSort,
        tags: &CrateTagFilter,
    ) -> DbResult<Vec<CrateOverview>> {
        Database::get_crate_overview_list(self, limit, offset, cache, visibility, sort, tags).await
    }

    async fn count_crate_overview_list(
        &self,
        cache: bool,
        visibility: &CrateVisibility,
        tags: &CrateTagFilter,
    ) -> DbResult<u64> {
        Database::count_crate_overview_list(self, cache, visibility, tags).await
    }

    async fn get_crate_data(&self, crate_name: &CanonicalName) -> DbResult<CrateData> {
        Database::get_crate_data(self, crate_name).await
    }

    async fn get_crate_dependents(&self, crate_name: &CanonicalName) -> DbResult<CrateDependents> {
        Database::get_crate_dependents(self, crate_name).await
    }

    async fn add_empty_crate(&self, name: &str, created: &DateTime<Utc>) -> DbResult<CrateId> {
        Database::add_empty_crate(self, name, created).await
    }

    async fn add_crate(
        &self,
        pub_metadata: &PublishMetadata,
        cksum: &str,
        size: i64,
        created: &DateTime<Utc>,
        owner: &UserName,
        token_name: Option<String>,
    ) -> DbResult<CrateId> {
        Database::add_crate(self, pub_metadata, cksum, size, created, owner, token_name).await
    }

    async fn update_docs_link(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
        docs_link: &str,
    ) -> DbResult<()> {
        Database::update_docs_link(self, crate_name, version, docs_link).await
    }

    async fn get_prefetch_data(&self, crate_name: &OriginalName) -> DbResult<Prefetch> {
        Database::get_prefetch_data(self, crate_name).await
    }

    async fn is_cratesio_cache_up_to_date(
        &self,
        crate_name: &OriginalName,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> DbResult<PrefetchState> {
        Database::is_cratesio_cache_up_to_date(self, crate_name, etag, last_modified).await
    }

    async fn add_cratesio_prefetch_data(
        &self,
        crate_name: &OriginalName,
        etag: &str,
        last_modified: &str,
        description: Option<String>,
        indices: &[IndexMetadata],
        licenses: &HashMap<String, String>,
    ) -> DbResult<Prefetch> {
        Database::add_cratesio_prefetch_data(
            self,
            crate_name,
            etag,
            last_modified,
            description,
            indices,
            licenses,
        )
        .await
    }

    async fn get_cratesio_index_update_list(&self) -> DbResult<Vec<CratesioPrefetchMsg>> {
        Database::get_cratesio_index_update_list(self).await
    }

    async fn get_cratesio_version_meta(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
    ) -> DbResult<Option<CratesioVersionMeta>> {
        Database::get_cratesio_version_meta(self, crate_name, version).await
    }

    async fn get_crate_license(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
    ) -> DbResult<Option<String>> {
        Database::get_crate_license(self, crate_name, version).await
    }

    async fn unyank_crate(&self, crate_name: &CanonicalName, version: &Version) -> DbResult<()> {
        Database::unyank_crate(self, crate_name, version).await
    }

    async fn yank_crate(&self, crate_name: &CanonicalName, version: &Version) -> DbResult<()> {
        Database::yank_crate(self, crate_name, version).await
    }

    async fn register_webhook(&self, webhook: Webhook) -> DbResult<String> {
        Database::register_webhook(self, webhook).await
    }

    async fn delete_webhook(&self, id: &str) -> DbResult<()> {
        Database::delete_webhook(self, id).await
    }

    async fn get_webhook(&self, id: &str) -> DbResult<Webhook> {
        Database::get_webhook(self, id).await
    }

    async fn get_all_webhooks(&self) -> DbResult<Vec<Webhook>> {
        Database::get_all_webhooks(self).await
    }

    async fn add_webhook_queue(
        &self,
        event: WebhookEvent,
        payload: serde_json::Value,
    ) -> DbResult<()> {
        Database::add_webhook_queue(self, event, payload).await
    }

    async fn get_pending_webhook_queue_entries(
        &self,
        timestamp: DateTime<Utc>,
    ) -> DbResult<Vec<WebhookQueue>> {
        Database::get_pending_webhook_queue_entries(self, timestamp).await
    }

    async fn update_webhook_queue(
        &self,
        id: &str,
        last_attempt: DateTime<Utc>,
        next_attempt: DateTime<Utc>,
    ) -> DbResult<()> {
        Database::update_webhook_queue(self, id, last_attempt, next_attempt).await
    }

    async fn delete_webhook_queue(&self, id: &str) -> DbResult<()> {
        Database::delete_webhook_queue(self, id).await
    }

    async fn get_user_by_oauth2_identity(
        &self,
        issuer: &str,
        subject: &str,
    ) -> DbResult<Option<User>> {
        Database::get_user_by_oauth2_identity(self, issuer, subject).await
    }

    async fn has_oauth2_admin(&self) -> DbResult<bool> {
        Database::has_oauth2_admin(self).await
    }

    async fn create_oauth2_user(
        &self,
        username: &UserName,
        issuer: &str,
        subject: &str,
        email: Option<String>,
        flags: UserFlags,
    ) -> DbResult<User> {
        Database::create_oauth2_user(self, username, issuer, subject, email, flags).await
    }

    async fn store_oauth2_state(
        &self,
        state: &str,
        pkce_verifier: &str,
        nonce: &str,
    ) -> DbResult<()> {
        Database::store_oauth2_state(self, state, pkce_verifier, nonce).await
    }

    async fn get_and_delete_oauth2_state(&self, state: &str) -> DbResult<Option<OAuth2StateData>> {
        Database::get_and_delete_oauth2_state(self, state).await
    }

    async fn cleanup_expired_oauth2_states(&self) -> DbResult<u64> {
        Database::cleanup_expired_oauth2_states(self).await
    }

    async fn is_username_available(&self, username: &UserName) -> DbResult<bool> {
        Database::is_username_available(self, username).await
    }

    async fn add_toolchain(
        &self,
        name: &str,
        version: &str,
        date: &str,
        channel: Option<String>,
    ) -> DbResult<i64> {
        Database::add_toolchain(self, name, version, date, channel).await
    }

    async fn add_toolchain_target(
        &self,
        toolchain_id: i64,
        target: &str,
        storage_path: &str,
        hash: &str,
        size: i64,
    ) -> DbResult<i64> {
        Database::add_toolchain_target(self, toolchain_id, target, storage_path, hash, size).await
    }

    async fn set_target_status(
        &self,
        target_id: i64,
        status: ToolchainTargetStatus,
    ) -> DbResult<()> {
        Database::set_target_status(self, target_id, status).await
    }

    async fn fail_processing_toolchain_targets(&self) -> DbResult<u64> {
        Database::fail_processing_toolchain_targets(self).await
    }

    async fn add_toolchain_component(
        &self,
        target_id: i64,
        name: &str,
        storage_path: &str,
        hash: &str,
        size: i64,
    ) -> DbResult<()> {
        Database::add_toolchain_component(self, target_id, name, storage_path, hash, size).await
    }

    async fn is_toolchain_component_stored_at(&self, storage_path: &str) -> DbResult<bool> {
        Database::is_toolchain_component_stored_at(self, storage_path).await
    }

    async fn get_toolchain_by_channel(
        &self,
        channel: &str,
    ) -> DbResult<Option<ToolchainWithTargets>> {
        Database::get_toolchain_by_channel(self, channel).await
    }

    async fn get_toolchain_by_version(
        &self,
        name: &str,
        version: &str,
    ) -> DbResult<Option<ToolchainWithTargets>> {
        Database::get_toolchain_by_version(self, name, version).await
    }

    async fn list_toolchains(&self) -> DbResult<Vec<ToolchainWithTargets>> {
        Database::list_toolchains(self).await
    }

    async fn delete_toolchain(&self, name: &str, version: &str) -> DbResult<()> {
        Database::delete_toolchain(self, name, version).await
    }

    async fn delete_toolchain_target(
        &self,
        name: &str,
        version: &str,
        target: &str,
    ) -> DbResult<()> {
        Database::delete_toolchain_target(self, name, version, target).await
    }

    async fn set_channel(&self, channel: &str, name: &str, version: &str) -> DbResult<()> {
        Database::set_channel(self, channel, name, version).await
    }

    async fn get_channels(&self) -> DbResult<Vec<ChannelInfo>> {
        Database::get_channels(self).await
    }

    async fn get_crate_original_name(
        &self,
        crate_name: &CanonicalName,
    ) -> DbResult<Option<OriginalName>> {
        Database::get_crate_original_name(self, crate_name).await
    }

    async fn get_original_name(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
    ) -> DbResult<Option<OriginalName>> {
        Database::get_original_name(self, crate_name, version).await
    }
}

/// Converts a count to the type of the database columns, saturating as no
/// realistic count gets that large.
fn saturating_i64(count: u64) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

/// The version if it is a valid semantic version. Invalid versions are logged,
/// so callers can skip them instead of failing on the whole crate.
fn valid_version(value: &str, crate_name: &str) -> Option<Version> {
    Version::try_from(value)
        .inspect_err(|_| warn!("Skipping invalid version {value} of crate {crate_name}"))
        .ok()
}

fn parse_db_version(value: &str) -> DbResult<Version> {
    Version::try_from(value).map_err(|_| DbError::InvalidVersion(value.to_owned()))
}

#[cfg(test)]
mod session_age_tests {
    use kellnr_settings::constants::MAX_SESSION_AGE;

    use super::{SessionAge, session_age_to_duration};

    #[test]
    fn zero_means_never_expire() {
        assert_eq!(session_age_to_duration(SessionAge::from_secs(0)), None);
    }

    #[test]
    fn positive_age_is_preserved() {
        assert_eq!(
            session_age_to_duration(SessionAge::from_secs(3600)),
            Some(chrono::Duration::seconds(3600))
        );
    }

    #[test]
    fn absurd_age_is_clamped_without_panicking() {
        // u64::MAX seconds would overflow chrono::Duration if not clamped.
        let resolved = session_age_to_duration(SessionAge::from_secs(u64::MAX))
            .expect("non-zero age yields Some");
        assert_eq!(
            resolved,
            chrono::Duration::from_std(MAX_SESSION_AGE).unwrap()
        );
    }
}
