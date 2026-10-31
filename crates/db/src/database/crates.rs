use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use kellnr_common::account_name::UserName;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::crate_data::{CrateData, CrateRegistryDep, CrateVersionData};
use kellnr_common::crate_dependents::{CrateDependent, CrateDependents};
use kellnr_common::index_metadata::IndexDep;
use kellnr_common::original_name::OriginalName;
use kellnr_common::prefetch::Prefetch;
use kellnr_common::publish_metadata::PublishMetadata;
use kellnr_common::version::{Version, latest_version};
use kellnr_entity::{
    crate_author, crate_author_to_crate, crate_category, crate_category_to_crate, crate_dependency,
    crate_index, crate_keyword, crate_keyword_to_crate, crate_meta, krate, owner, user,
};
use kellnr_migration::iden::CrateIden;
use sea_orm::query::{QueryOrder, QuerySelect};
use sea_orm::sea_query::{Cond, Expr, JoinType};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, ExprTrait, ModelTrait, QueryFilter,
    RelationTrait, Set,
};

use super::{DB_DATE_FORMAT, Database, operations, parse_db_version, valid_version};
use crate::error::DbError;
use crate::provider::{CrateVisibility, DbResult};
use crate::{CrateId, CrateMeta};

impl Database {
    /// The error for a crate that could not be created, as a concurrent
    /// publish created one with the same canonical name.
    async fn concurrent_name_conflict(&self, name: &OriginalName) -> DbError {
        match krate::Entity::find()
            .filter(krate::Column::Name.eq(name.to_canonical()))
            .one(&self.db_con)
            .await
        {
            Ok(Some(krate)) => operations::name_conflict(name, krate.original_name),
            Ok(None) => operations::name_conflict(name, name.to_string()),
            Err(e) => e.into(),
        }
    }

    /// Looks up a crate index by name and version; returns the entity model or [`DbError::CrateIndexNotFound`].
    /// Yanks or unyanks a version and updates the `ETag` of the crate index,
    /// so clients fetch the changed index.
    async fn set_yanked(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
        yanked: bool,
    ) -> DbResult<()> {
        self.in_transaction(async |txn| {
            // The index stores the original name, so look it up by the crate
            let ci = crate_index::Entity::find()
                .inner_join(krate::Entity)
                .filter(krate::Column::Name.eq(crate_name))
                .filter(crate_index::Column::Vers.eq(version))
                .one(txn)
                .await?
                .ok_or_else(|| {
                    DbError::CrateIndexNotFound(crate_name.to_string(), version.to_string())
                })?;
            let crate_id = ci.crate_fk;
            let mut ci: crate_index::ActiveModel = ci.into();
            ci.yanked = Set(yanked);
            ci.update(txn).await?;
            operations::update_etag(txn, crate_name.as_str(), crate_id).await?;
            Ok(())
        })
        .await
    }

    pub(super) async fn get_last_updated_crate(
        &self,
        visibility: &CrateVisibility,
    ) -> DbResult<Option<(OriginalName, Version)>> {
        let mut query = krate::Entity::find();
        if let Some(condition) = Self::crate_visibility_condition(visibility) {
            query = query.filter(condition);
        }
        let krate = query
            .order_by_desc(krate::Column::LastUpdated)
            .one(&self.db_con)
            .await?;

        if let Some(krate) = krate {
            // SAFETY: Unchecked is ok, as only valid crate names are inserted into the database
            let name = OriginalName::from_unchecked(krate.original_name);
            let version = parse_db_version(&krate.max_version)?;
            Ok(Some((name, version)))
        } else {
            Ok(None)
        }
    }

    pub(super) async fn get_crate_id(
        &self,
        crate_name: &CanonicalName,
    ) -> DbResult<Option<CrateId>> {
        let id = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name))
            .one(&self.db_con)
            .await?
            .map(|model| CrateId::new(model.id));

        Ok(id)
    }

    pub(super) async fn get_crate_versions(
        &self,
        crate_name: &CanonicalName,
    ) -> DbResult<Vec<Version>> {
        let u = crate_meta::Entity::find()
            .join(JoinType::InnerJoin, crate_meta::Relation::Krate.def())
            .filter(Expr::col((CrateIden::Table, krate::Column::Name)).eq(crate_name))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter()
            .filter_map(|meta| valid_version(&meta.version, crate_name.as_str()))
            .collect())
    }

    pub(super) async fn crate_version_exists(
        &self,
        crate_id: CrateId,
        version: &Version,
    ) -> DbResult<bool> {
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

    pub(super) async fn get_max_version_from_id(&self, crate_id: CrateId) -> DbResult<Version> {
        operations::get_max_version_from_id(&self.db_con, crate_id.get()).await
    }

    pub(super) async fn get_max_version_from_name(
        &self,
        crate_name: &CanonicalName,
    ) -> DbResult<Version> {
        let k = self.get_krate_model(crate_name).await?;
        let v = Version::try_from(&k.max_version)
            .map_err(|_| DbError::FailedToGetMaxVersionByName(crate_name.to_string()))?;
        Ok(v)
    }

    pub(super) async fn delete_crate(
        &self,
        krate: &CanonicalName,
        version: &Version,
    ) -> DbResult<()> {
        self.in_transaction(async |txn| {
            // Delete the entry from the "crate_meta" table
            let crate_meta_version = crate_meta::Entity::find()
                .join(JoinType::InnerJoin, crate_meta::Relation::Krate.def())
                .filter(krate::Column::Name.eq(krate))
                .filter(crate_meta::Column::Version.eq(version))
                .one(txn)
                .await?
                .ok_or_else(|| {
                    DbError::CrateMetaNotFound(krate.to_string(), version.to_string())
                })?;
            let crate_id = crate_meta_version.crate_fk;
            let current_max_version = operations::get_max_version_from_id(txn, crate_id).await?;
            crate_meta_version.delete(txn).await?;

            // Delete the crate index entry from "crate_index" table
            let crate_index_version = crate_index::Entity::find()
                .join(JoinType::InnerJoin, crate_index::Relation::Krate.def())
                .filter(krate::Column::Name.eq(krate))
                .filter(crate_index::Column::Vers.eq(version))
                .one(txn)
                .await?
                .ok_or_else(|| {
                    DbError::CrateIndexNotFound(krate.to_string(), version.to_string())
                })?;
            crate_index_version.delete(txn).await?;

            // If it was the last entry in the "crate_meta" table, delete the entry
            // in the "crate" table as well
            let crate_meta_rows = crate_meta::Entity::find()
                .join(JoinType::InnerJoin, crate_meta::Relation::Krate.def())
                .filter(krate::Column::Name.eq(krate))
                .all(txn)
                .await?;

            if crate_meta_rows.is_empty() {
                krate::Entity::delete_many()
                    .filter(krate::Column::Name.eq(krate))
                    .exec(txn)
                    .await?;
            } else {
                let c = krate::Entity::find_by_id(crate_id)
                    .one(txn)
                    .await?
                    .ok_or(DbError::CrateNotFoundWithId(crate_id))?;
                let mut c: krate::ActiveModel = c.into();

                // Update the max. version if the deleted version was the max. version.
                if version == &current_max_version {
                    let new_max_version = crate_meta_rows
                        .into_iter()
                        .filter_map(|cm| valid_version(&cm.version, krate.as_str()))
                        .max()
                        .ok_or(DbError::FailedToGetMaxVersionById(crate_id))?;
                    c.max_version = Set(new_max_version.to_string());
                }
                // Update the ETag value of the crate index.
                let etag = operations::compute_etag(txn, krate.as_str(), crate_id).await?;
                c.e_tag = Set(etag);
                c.update(txn).await?;
            }

            Ok(())
        })
        .await
    }

    pub(super) async fn get_crate_meta_list(
        &self,
        crate_name: &CanonicalName,
    ) -> DbResult<Vec<CrateMeta>> {
        let Some(krate) = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name))
            .one(&self.db_con)
            .await?
        else {
            return Ok(Vec::new());
        };
        let crate_meta = krate
            .find_related(crate_meta::Entity)
            .all(&self.db_con)
            .await?;

        let name = krate.original_name.to_lowercase();
        let crate_meta = crate_meta
            .into_iter()
            .map(|cm| CrateMeta {
                name: name.clone(),
                id: cm.id,
                version: cm.version,
                created: cm.created,
                downloads: cm.downloads,
                crate_fk: cm.crate_fk,
            })
            .collect();

        Ok(crate_meta)
    }

    pub(super) async fn get_crate_data(&self, crate_name: &CanonicalName) -> DbResult<CrateData> {
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

        // The dependencies of all versions, looked up together
        let mut index_deps: HashMap<&str, Vec<IndexDep>> = HashMap::new();
        for ci in &crate_indices {
            if let Some(deps) = ci.deps.clone() {
                let deps = serde_json::from_value::<Vec<IndexDep>>(deps)
                    .map_err(|e| DbError::FailedToConvertToJson(e.to_string()))?;
                index_deps.insert(ci.vers.as_str(), deps);
            }
        }
        let dep_info =
            operations::get_dep_info(&self.db_con, index_deps.values().flatten()).await?;

        let mut versions = Vec::new();
        for cm in crate_metas {
            let ci = crate_indices
                .iter()
                .find(|ci| ci.vers == cm.version)
                .ok_or_else(|| {
                    DbError::CrateIndexNotFound(krate.name.clone(), cm.version.clone())
                })?;
            let dependencies: Vec<CrateRegistryDep> = index_deps
                .get(ci.vers.as_str())
                .into_iter()
                .flatten()
                .map(|dep| {
                    let info = operations::dep_lookup_name(dep)
                        .and_then(|name| dep_info.get(name.as_str()))
                        .cloned()
                        .unwrap_or_default();
                    CrateRegistryDep::from_index(info.description, info.latest_version, dep.clone())
                })
                .collect();
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
                size: cm.size,
                rust_version: ci.rust_version.clone(),
                publisher: cm.publisher.clone(),
                publisher_token: cm.publisher_token.clone(),
            });
        }
        // Newest first, invalid versions last
        versions.sort_by_cached_key(|v| Reverse(valid_version(&v.version, &krate.name)));

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

    pub(super) async fn get_crate_dependents(
        &self,
        crate_name: &CanonicalName,
    ) -> DbResult<CrateDependents> {
        let krate = self.get_krate_model(crate_name).await?;

        // Requirements on the crate, by the non-yanked version that has them
        let requirements: Vec<(i64, String, String, String)> = crate_dependency::Entity::find()
            .select_only()
            .column(crate_index::Column::CrateFk)
            .column(crate_index::Column::Vers)
            .column(crate_dependency::Column::VersionReq)
            .column(crate_dependency::Column::Kind)
            .inner_join(crate_index::Entity)
            .filter(crate_dependency::Column::Name.eq(crate_name))
            .filter(crate_index::Column::Yanked.eq(false))
            .filter(crate_index::Column::CrateFk.ne(krate.id))
            .into_tuple()
            .all(&self.db_con)
            .await?;
        let mut requirements_by_version: HashMap<(i64, String), Vec<(String, String)>> =
            HashMap::new();
        for (crate_fk, vers, version_req, kind) in requirements {
            requirements_by_version
                .entry((crate_fk, vers))
                .or_default()
                .push((version_req, kind));
        }

        // Only the latest version of a dependent counts, so the versions of
        // the crate and of every crate that ever depended on it are needed
        let mut crate_ids: Vec<i64> = requirements_by_version
            .keys()
            .map(|(crate_fk, _)| *crate_fk)
            .collect();
        crate_ids.push(krate.id);
        crate_ids.sort_unstable();
        crate_ids.dedup();
        let mut versions_by_crate: HashMap<i64, (String, Vec<String>)> = HashMap::new();
        // Chunked to stay below the bind parameter limit of SQLite
        for ids in crate_ids.chunks(1000) {
            let versions: Vec<(i64, String, String)> = crate_index::Entity::find()
                .select_only()
                .column(crate_index::Column::CrateFk)
                .column(krate::Column::OriginalName)
                .column(crate_index::Column::Vers)
                .inner_join(krate::Entity)
                .filter(crate_index::Column::CrateFk.is_in(ids.iter().copied()))
                .filter(crate_index::Column::Yanked.eq(false))
                .into_tuple()
                .all(&self.db_con)
                .await?;
            for (crate_fk, name, vers) in versions {
                versions_by_crate
                    .entry(crate_fk)
                    .or_insert_with(|| (name, Vec::new()))
                    .1
                    .push(vers);
            }
        }

        let latest = versions_by_crate
            .remove(&krate.id)
            .and_then(|(_, versions)| latest_version(versions.iter().map(String::as_str)));
        let mut dependents: Vec<CrateDependent> = versions_by_crate
            .into_iter()
            .filter_map(|(crate_fk, (name, versions))| {
                let version = latest_version(versions.iter().map(String::as_str))?;
                let requirements = requirements_by_version.remove(&(crate_fk, version.clone()))?;
                Some(CrateDependent::new(
                    name,
                    version,
                    requirements,
                    latest.as_deref(),
                ))
            })
            .collect();
        dependents.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(CrateDependents {
            latest_version: latest,
            dependents,
            restricted: 0,
        })
    }

    pub(super) async fn add_empty_crate(
        &self,
        name: &str,
        created: &DateTime<Utc>,
    ) -> DbResult<CrateId> {
        let created = created.format(DB_DATE_FORMAT).to_string();
        let original_name = OriginalName::try_from(name)
            .map_err(|_| DbError::InvalidCrateName(name.to_string()))?;
        operations::ensure_no_similar_crate(&self.db_con, &original_name).await?;
        let krate = krate::ActiveModel {
            id: ActiveValue::default(),
            name: Set(original_name.to_canonical().into_inner()),
            original_name: Set(name.to_string()),
            max_version: Set("0.0.0".to_string()),
            last_updated: Set(created),
            total_downloads: Set(0),
            homepage: Set(None),
            description: Set(None),
            repository: Set(None),
            e_tag: Set(String::new()), // Set to empty string, as it can be computed, when the crate index is inserted
            restricted_download: Set(false),
        };
        match krate.insert(&self.db_con).await {
            Ok(krate) => Ok(CrateId::new(krate.id)),
            Err(e) if operations::is_unique_violation(&e) => {
                Err(self.concurrent_name_conflict(&original_name).await)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub(super) async fn add_crate(
        &self,
        pub_metadata: &PublishMetadata,
        cksum: &str,
        size: i64,
        created: &DateTime<Utc>,
        owner: &UserName,
        token_name: Option<String>,
    ) -> DbResult<CrateId> {
        let created = created.format(DB_DATE_FORMAT).to_string();
        let original_name = OriginalName::try_from(&pub_metadata.name)
            .map_err(|_| DbError::InvalidCrateName(pub_metadata.name.clone()))?;
        let canonical_name = original_name.to_canonical();

        // Look up the crate by its canonical name. The transaction keeps the
        // publish atomic, but does not serialize concurrent first publishes.
        // Those fail on the unique crate name. The row lock serializes
        // publishes of an existing crate on PostgreSQL, so the version check
        // below sees concurrently published versions.
        let mut name_taken = false;
        let result = self
            .in_transaction(async |txn| {
                let existing = krate::Entity::find()
                    .filter(krate::Column::Name.eq(&canonical_name))
                    .lock_exclusive()
                    .one(txn)
                    .await?;

                let version = parse_db_version(&pub_metadata.vers)?;
                let (crate_id, is_newest) = if let Some(krate) = existing {
                    let krate_id = krate.id;
                    operations::ensure_same_separators(&original_name, &krate.original_name)?;
                    operations::ensure_owner_or_admin(
                        txn,
                        owner,
                        &original_name.to_lowercase(),
                        krate_id,
                    )
                    .await?;
                    // Like crates.io, the name must be spelled like on the first publish,
                    // else the index and the storage would use different spellings.
                    if krate.original_name != pub_metadata.name {
                        return Err(DbError::CrateNameMismatch(
                            pub_metadata.name.clone(),
                            krate.original_name,
                        ));
                    }
                    if let Some(existing) =
                        operations::find_version_ignoring_build(txn, krate_id, &version).await?
                    {
                        return Err(DbError::CrateVersionExists(
                            pub_metadata.name.clone(),
                            existing,
                        ));
                    }
                    // The crate level metadata is the one of the newest version, so a
                    // fix release of an older version must not overwrite it.
                    let is_newest = version >= parse_db_version(&krate.max_version)?;

                    let mut krate: krate::ActiveModel = krate.into();
                    krate.last_updated = Set(created.clone());
                    if is_newest {
                        krate.max_version = Set(version.to_string());
                        krate.homepage = Set(pub_metadata.homepage.clone());
                        krate.description = Set(pub_metadata.description.clone());
                        krate.repository = Set(pub_metadata.repository.clone());
                    }
                    krate.e_tag = Set(String::new()); // Set to empty string, as it can be computed, when the crate index is inserted
                    krate.update(txn).await?;
                    (krate_id, is_newest)
                } else {
                    let krate = krate::ActiveModel {
                        id: ActiveValue::default(),
                        name: Set(canonical_name.into_inner()),
                        original_name: Set(pub_metadata.name.clone()),
                        max_version: Set(version.to_string()),
                        last_updated: Set(created.clone()),
                        total_downloads: Set(0),
                        homepage: Set(pub_metadata.homepage.clone()),
                        description: Set(pub_metadata.description.clone()),
                        repository: Set(pub_metadata.repository.clone()),
                        e_tag: Set(String::new()), // Set to empty string, as it can be computed, when the crate index is inserted
                        restricted_download: Set(false),
                    };
                    let krate = match krate.insert(txn).await {
                        Ok(krate) => krate,
                        Err(e) if operations::is_unique_violation(&e) => {
                            name_taken = true;
                            return Err(e.into());
                        }
                        Err(e) => return Err(e.into()),
                    };
                    (krate.id, true)
                };

                operations::add_owner_if_not_exists(txn, owner, crate_id).await?;
                operations::add_crate_metadata(
                    txn,
                    pub_metadata,
                    size,
                    &created,
                    crate_id,
                    Some(owner.as_str()),
                    token_name.as_deref(),
                )
                .await?;
                operations::add_crate_index(txn, pub_metadata, cksum, crate_id).await?;
                operations::update_etag(txn, &pub_metadata.name, crate_id).await?;
                if is_newest {
                    operations::update_crate_categories(txn, pub_metadata, crate_id).await?;
                    operations::update_crate_keywords(txn, pub_metadata, crate_id).await?;
                    operations::update_crate_authors(txn, pub_metadata, crate_id).await?;
                }

                Ok(CrateId::new(crate_id))
            })
            .await;
        if name_taken {
            return Err(self.concurrent_name_conflict(&original_name).await);
        }
        result
    }

    pub(super) async fn update_docs_link(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
        docs_link: &str,
    ) -> DbResult<()> {
        let (cm, _c) = crate_meta::Entity::find()
            .find_also_related(krate::Entity)
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name))
                    .add(crate_meta::Column::Version.eq(version)),
            )
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?;

        let mut cm: crate_meta::ActiveModel = cm.into();
        cm.documentation = Set(Some(docs_link.to_string()));
        cm.update(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn get_prefetch_data(&self, crate_name: &OriginalName) -> DbResult<Prefetch> {
        // Like the index of crates.io, the index file is only found under the
        // separators the crate was published with.
        let krate = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_canonical()))
            .one(&self.db_con)
            .await?
            .filter(|krate| krate.original_name.to_lowercase() == crate_name.to_lowercase())
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?;
        let crate_indices = operations::crate_indices(&self.db_con, krate.id).await?;

        let index_metadata =
            operations::crate_index_model_to_index_metadata(crate_name, crate_indices)?;
        let data = operations::index_metadata_to_bytes(&index_metadata)?;

        Ok(Prefetch {
            data,
            etag: krate.e_tag,
            last_modified: krate.last_updated,
        })
    }

    pub(super) async fn get_crate_license(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
    ) -> DbResult<Option<String>> {
        let meta = crate_meta::Entity::find()
            .inner_join(krate::Entity)
            .filter(krate::Column::Name.eq(crate_name))
            .filter(crate_meta::Column::Version.eq(version))
            .one(&self.db_con)
            .await?;
        Ok(meta.and_then(|m| m.license))
    }

    pub(super) async fn unyank_crate(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
    ) -> DbResult<()> {
        self.set_yanked(crate_name, version, false).await
    }

    pub(super) async fn yank_crate(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
    ) -> DbResult<()> {
        self.set_yanked(crate_name, version, true).await
    }

    pub(super) async fn get_crate_original_name(
        &self,
        crate_name: &CanonicalName,
    ) -> DbResult<Option<OriginalName>> {
        Ok(krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name))
            .one(&self.db_con)
            .await?
            .map(|k| OriginalName::from_unchecked(k.original_name)))
    }

    pub(super) async fn get_original_name(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
    ) -> DbResult<Option<OriginalName>> {
        let found = crate_meta::Entity::find()
            .find_also_related(krate::Entity)
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name))
                    .add(crate_meta::Column::Version.eq(version)),
            )
            .one(&self.db_con)
            .await?;
        Ok(found
            .and_then(|(_, krate)| krate)
            .map(|krate| OriginalName::from_unchecked(krate.original_name)))
    }
}
