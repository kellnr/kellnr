use crate::{crate_meta, error::DbError, AuthToken, CrateSummary, DocQueueEntry, User};
use chrono::{DateTime, Utc};
use common::crate_data::CrateData;
use common::crate_overview::CrateOverview;
use common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use common::index_metadata::IndexMetadata;
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::prefetch::Prefetch;
use common::publish_metadata::PublishMetadata;
use common::version::Version;
use crate_meta::CrateMeta;
use sea_orm::prelude::async_trait::async_trait;
use std::path::Path;

pub type DbResult<T> = Result<T, DbError>;
#[derive(Debug, PartialEq, Eq)]
pub enum PrefetchState {
    UpToDate,
    NeedsUpdate(Prefetch),
    NotFound,
}

#[async_trait]
pub trait DbProvider: Send + Sync {
    async fn get_last_updated_crate(&self) -> DbResult<Option<(OriginalName, Version)>>;
    async fn authenticate_user(&self, name: &str, pwd: &str) -> DbResult<User>;
    async fn increase_download_counter(
        &self,
        crate_name: &NormalizedName,
        crate_version: &Version,
    ) -> DbResult<()>;
    async fn increase_cached_download_counter(
        &self,
        crate_name: &NormalizedName,
        crate_version: &Version,
    ) -> DbResult<()>;
    async fn validate_session(&self, session_token: &str) -> DbResult<(String, bool)>;
    async fn add_session_token(&self, name: &str, session_token: &str) -> DbResult<()>;
    async fn add_crate_user(&self, crate_name: &NormalizedName, user: &str) -> DbResult<()>;
    async fn add_owner(&self, crate_name: &NormalizedName, owner: &str) -> DbResult<()>;
    async fn is_download_restricted(&self, crate_name: &NormalizedName) -> DbResult<bool>;
    async fn change_download_restricted(
        &self,
        crate_name: &NormalizedName,
        restricted: bool,
    ) -> DbResult<()>;
    async fn is_crate_user(&self, crate_name: &NormalizedName, user: &str) -> DbResult<bool>;
    async fn is_owner(&self, crate_name: &NormalizedName, user: &str) -> DbResult<bool>;
    async fn get_crate_id(&self, crate_name: &NormalizedName) -> DbResult<Option<i64>>;
    async fn get_crate_owners(&self, crate_name: &NormalizedName) -> DbResult<Vec<User>>;
    async fn get_crate_users(&self, crate_name: &NormalizedName) -> DbResult<Vec<User>>;
    async fn delete_session_token(&self, session_token: &str) -> DbResult<()>;
    async fn delete_user(&self, user_name: &str) -> DbResult<()>;
    async fn change_pwd(&self, user_name: &str, new_pwd: &str) -> DbResult<()>;
    async fn crate_version_exists(&self, crate_id: i64, version: &str) -> DbResult<bool>;
    async fn get_max_version_from_id(&self, crate_id: i64) -> DbResult<Version>;
    async fn get_max_version_from_name(&self, crate_name: &NormalizedName) -> DbResult<Version>;
    async fn update_max_version(&self, crate_id: i64, version: &Version) -> DbResult<()>;
    async fn add_auth_token(&self, name: &str, token: &str, user: &str) -> DbResult<()>;
    async fn get_user_from_token(&self, token: &str) -> DbResult<User>;
    async fn get_user(&self, name: &str) -> DbResult<User>;
    async fn get_auth_tokens(&self, user_name: &str) -> DbResult<Vec<AuthToken>>;
    async fn delete_auth_token(&self, id: i32) -> DbResult<()>;
    async fn delete_owner(&self, crate_name: &str, owner: &str) -> DbResult<()>;
    async fn delete_crate_user(&self, crate_name: &str, user: &str) -> DbResult<()>;
    async fn add_user(&self, name: &str, pwd: &str, salt: &str, is_admin: bool) -> DbResult<()>;
    async fn get_users(&self) -> DbResult<Vec<User>>;
    async fn get_total_unique_crates(&self) -> DbResult<u32>;
    async fn get_total_crate_versions(&self) -> DbResult<u32>;
    async fn get_total_downloads(&self) -> DbResult<u64>;
    async fn get_top_crates_downloads(&self, top: u32) -> DbResult<Vec<(String, u64)>>;
    async fn get_total_unique_cached_crates(&self) -> DbResult<u64>;
    async fn get_total_cached_crate_versions(&self) -> DbResult<u64>;
    async fn get_total_cached_downloads(&self) -> DbResult<u64>;
    async fn get_crate_summaries(&self) -> DbResult<Vec<CrateSummary>>;
    async fn add_doc_queue(
        &self,
        krate: &NormalizedName,
        version: &Version,
        path: &Path,
    ) -> DbResult<()>;
    async fn delete_doc_queue(&self, id: i64) -> DbResult<()>;
    async fn get_doc_queue(&self) -> DbResult<Vec<DocQueueEntry>>;
    async fn delete_crate(&self, krate: &NormalizedName, version: &Version) -> DbResult<()>;
    async fn get_crate_meta_list(&self, crate_name: &NormalizedName) -> DbResult<Vec<CrateMeta>>;
    async fn update_last_updated(&self, id: i64, last_updated: &DateTime<Utc>) -> DbResult<()>;
    async fn search_in_crate_name(
        &self,
        contains: &str,
        cache: bool,
    ) -> DbResult<Vec<CrateOverview>>;
    async fn get_crate_overview_list(
        &self,
        limit: u64,
        offset: u64,
        cache: bool,
    ) -> DbResult<Vec<CrateOverview>>;
    async fn get_crate_data(&self, crate_name: &NormalizedName) -> DbResult<CrateData>;
    async fn add_crate(
        &self,
        pub_metadata: &PublishMetadata,
        cksum: &str,
        created: &DateTime<Utc>,
        owner: &str,
    ) -> DbResult<i64>;
    async fn update_docs_link(
        &self,
        crate_name: &NormalizedName,
        version: &Version,
        docs_link: &str,
    ) -> DbResult<()>;
    async fn add_crate_metadata(
        &self,
        pub_metadata: &PublishMetadata,
        created: &str,
        crate_id: i64,
    ) -> DbResult<()>;
    async fn get_prefetch_data(&self, crate_name: &str) -> DbResult<Prefetch>;
    async fn is_cratesio_cache_up_to_date(
        &self,
        crate_name: &NormalizedName,
        if_none_match: Option<String>,
        if_modified_since: Option<String>,
    ) -> DbResult<PrefetchState>;
    async fn add_cratesio_prefetch_data(
        &self,
        crate_name: &OriginalName,
        etag: &str,
        last_modified: &str,
        description: Option<String>,
        indices: &[IndexMetadata],
    ) -> DbResult<Prefetch>;
    async fn get_cratesio_index_update_list(&self) -> DbResult<Vec<CratesioPrefetchMsg>>;
    async fn unyank_crate(&self, crate_name: &NormalizedName, version: &Version) -> DbResult<()>;
    async fn yank_crate(&self, crate_name: &NormalizedName, version: &Version) -> DbResult<()>;
}

pub mod mock {
    use super::*;
    use chrono::DateTime;
    use mockall::predicate::*;
    use mockall::*;

    mock! {
          pub Db {}

        #[async_trait]
        impl DbProvider for Db {

            async fn get_last_updated_crate(&self) -> DbResult<Option<(OriginalName, Version)>> {
                unimplemented!()
            }

            async fn authenticate_user(&self, _name: &str, _pwd: &str) -> DbResult<User> {
                unimplemented!()
            }

            async fn increase_download_counter(
                &self,
                _crate_name: &NormalizedName,
                _crate_version: &Version,
            ) -> DbResult<()> {
                unimplemented!()
            }

            async fn increase_cached_download_counter(
                &self,
                _crate_name: &NormalizedName,
                _crate_version: &Version,
            ) -> DbResult<()> {
                unimplemented!()
            }

            async fn validate_session(&self, _session_token: &str) -> DbResult<(String, bool)> {
                unimplemented!()
            }

            async fn add_session_token(&self, _name: &str, _session_token: &str) -> DbResult<()> {
                unimplemented!()
            }

            async fn add_crate_user(&self, crate_name: &NormalizedName, user: &str) -> DbResult<()> {
                unimplemented!()
            }

            async fn add_owner(&self, _crate_name: &NormalizedName, _owner: &str) -> DbResult<()> {
                unimplemented!()
            }

            async fn is_download_restricted(&self, crate_name: &NormalizedName) -> DbResult<bool> {
                unimplemented!()
            }

            async fn change_download_restricted(
                &self,
                crate_name: &NormalizedName,
                restricted: bool,
            ) -> DbResult<()> {
                unimplemented!()
            }

            async fn is_crate_user(&self, _crate_name: &NormalizedName, _user: &str) -> DbResult<bool> {
                unimplemented!()
            }

            async fn is_owner(&self, _crate_name: &NormalizedName, _user: &str) -> DbResult<bool> {
                unimplemented!()
            }

            async fn get_crate_id(&self, _crate_name: &NormalizedName) -> DbResult<Option<i64>> {
                unimplemented!()
            }

            async fn get_crate_owners(&self, _crate_name: &NormalizedName) -> DbResult<Vec<User>> {
                unimplemented!()
            }

            async fn get_crate_users(&self, _crate_name: &NormalizedName) -> DbResult<Vec<User>> {
                unimplemented!()
            }

            async fn delete_session_token(&self, _session_token: &str) -> DbResult<()> {
                unimplemented!()
            }

            async fn delete_user(&self, _user_name: &str) -> DbResult<()> {
                unimplemented!()
            }

            async fn change_pwd(&self, _user_name: &str, _new_pwd: &str) -> DbResult<()> {
                unimplemented!()
            }

            async fn crate_version_exists(&self, _crate_id: i64, _version: &str) -> DbResult<bool> {
                unimplemented!()
            }

            async fn get_max_version_from_id(&self, crate_id: i64) -> DbResult<Version>  {
                unimplemented!()
            }

            async fn get_max_version_from_name(&self, crate_name: &NormalizedName) -> DbResult<Version> {
                unimplemented!()
            }

            async fn update_max_version(&self, crate_id: i64, version: &Version) -> DbResult<()> {
                unimplemented!()
            }

            async fn add_auth_token(&self, _name: &str, _token: &str, _user: &str) -> DbResult<()> {
                unimplemented!()
            }

            async fn get_user_from_token(&self, _token: &str) -> DbResult<User> {
                unimplemented!()
            }

            async fn get_user(&self, _name: &str) -> DbResult<User> {
                unimplemented!()
            }

            async fn get_auth_tokens(&self, _user_name: &str) -> DbResult<Vec<AuthToken>> {
                unimplemented!()
            }

            async fn delete_auth_token(&self, _id: i32) -> DbResult<()> {
                unimplemented!()
            }

            async fn delete_owner(&self, _crate_name: &str, _owner: &str) -> DbResult<()> {
                unimplemented!()
            }

            async fn delete_crate_user(&self, crate_name: &str, user: &str) -> DbResult<()>{
                unimplemented!()
            }

            async fn add_user(&self, _name: &str, _pwd: &str, _salt: &str, _is_admin: bool) -> DbResult<()> {
                unimplemented!()
            }

            async fn get_users(&self) -> DbResult<Vec<User>> {
                unimplemented!()
            }

            async fn get_total_unique_crates(&self) -> DbResult<u32> {
                unimplemented!()
            }

            async fn get_total_crate_versions(&self) -> DbResult<u32> {
                unimplemented!()
            }

            async fn get_top_crates_downloads(&self, _top: u32) -> DbResult<Vec<(String, u64)>> {
                unimplemented!()
            }

            async fn get_total_unique_cached_crates(&self) -> DbResult<u64> {
                unimplemented!()
            }

            async fn get_total_cached_crate_versions(&self) -> DbResult<u64> {
                unimplemented!()
            }

            async fn get_total_cached_downloads(&self) -> DbResult<u64> {
                unimplemented!()
            }

            async fn get_crate_summaries(&self) -> DbResult<Vec<CrateSummary >> {
                unimplemented!()
            }

                async fn add_doc_queue(&self, krate: &NormalizedName, version: &Version, path: &Path) -> DbResult<()>{
                    unimplemented!()
                }

            async fn delete_doc_queue(&self, id: i64) -> DbResult<()>{
                    unimplemented!()
                }

            async fn get_doc_queue(&self) -> DbResult<Vec<DocQueueEntry>> {
                unimplemented!()
            }

            async fn delete_crate(&self, krate: &NormalizedName, version: &Version) -> DbResult<()> {
                unimplemented!()
            }

            async fn get_total_downloads(&self) -> DbResult<u64>{
                unimplemented!()
            }

            async fn get_crate_meta_list(&self, crate_name: &NormalizedName) -> DbResult<Vec<CrateMeta>>{
                unimplemented!()
            }

            async fn update_last_updated(&self, id: i64, last_updated: &DateTime<Utc>) -> DbResult<()>{
                unimplemented!()
            }

            async fn search_in_crate_name(&self, contains: &str, cache: bool) -> DbResult<Vec<CrateOverview>> {
                unimplemented!()
            }

            async fn get_crate_overview_list(&self, limit: u64, offset: u64, cache: bool) -> DbResult<Vec<CrateOverview >> {
                unimplemented!()
            }

            async fn get_crate_data(&self, crate_name: &NormalizedName) -> DbResult<CrateData> {
                unimplemented!()
            }

            async fn add_crate(&self, pub_metadata: &PublishMetadata, sha256: &str, created: &DateTime<Utc>, owner: &str) -> DbResult<i64> {
                unimplemented!()
            }

            async fn update_docs_link(&self, crate_name: &NormalizedName, version: &Version, docs_link: &str) -> DbResult<()> {
                unimplemented!()
            }

            async fn add_crate_metadata(&self, pub_metadata: &PublishMetadata, created: &str, crate_id: i64,) -> DbResult<()> {
                unimplemented!()
            }

            async fn get_prefetch_data(&self, crate_name: &str) -> DbResult<Prefetch> {
                unimplemented!()
            }

            async fn is_cratesio_cache_up_to_date(&self, crate_name: &NormalizedName, etag: Option<String>, last_modified: Option<String>) -> DbResult<PrefetchState> {
                unimplemented!()
            }

            async fn add_cratesio_prefetch_data(
                &self,
                crate_name: &OriginalName,
                etag: &str,
                last_modified: &str,
                description: Option<String>,
                indices: &[IndexMetadata],
            ) -> DbResult<Prefetch> {
                unimplemented!()
            }

            async fn get_cratesio_index_update_list(&self) -> DbResult<Vec<CratesioPrefetchMsg>> {
                unimplemented!()
            }

            async fn unyank_crate(&self, crate_name: &NormalizedName, version: &Version) -> DbResult<()> {
                unimplemented!()
            }

            async fn yank_crate(&self, crate_name: &NormalizedName, version: &Version) -> DbResult<()> {
                unimplemented!()
            }
        }
    }
}
