//! Core database operations.
//!
//! Standalone async functions that take a database connection and perform
//! specific operations. Used by the Database impl to keep the main module
//! focused on the `DbProvider` trait implementation.

use std::collections::{HashMap, HashSet};

use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::crate_data::CRATESIO_INDEX;
use kellnr_common::crate_dependents::LocalDependency;
use kellnr_common::index_metadata::{IndexDep, IndexMetadata, schema_version, split_features};
use kellnr_common::original_name::OriginalName;
use kellnr_common::publish_metadata::PublishMetadata;
use kellnr_common::version::{Version, latest_version};
use kellnr_entity::{
    auth_token, crate_author, crate_author_to_crate, crate_category, crate_category_to_crate,
    crate_dependency, crate_index, crate_keyword, crate_keyword_to_crate, crate_meta,
    cratesio_crate, cratesio_index, krate, owner, user,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, SqlErr, TryInsertResult,
};

use crate::ConString;
use crate::error::DbError;
use crate::password::{hash_pwd, hash_token};
use crate::provider::DbResult;

/// What Kellnr knows about a dependency of a crate.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DepInfo {
    pub description: Option<String>,
    /// Latest version that is not yanked
    pub latest_version: Option<String>,
}

/// Name a dependency is looked up by: the package name for renamed
/// dependencies. `None` for names no crate can have.
pub fn dep_lookup_name(dep: &IndexDep) -> Option<CanonicalName> {
    CanonicalName::try_from(dep.package.as_deref().unwrap_or(&dep.name)).ok()
}

/// Description and latest version of the dependencies, by the canonical name
/// [`dep_lookup_name`] returns.
/// crates.io dependencies are only known if they are in the proxy cache.
pub async fn get_dep_info<'a, C, D>(db_con: &C, deps: D) -> DbResult<HashMap<String, DepInfo>>
where
    C: ConnectionTrait,
    D: Iterator<Item = &'a IndexDep>,
{
    let mut cratesio_names = HashSet::new();
    let mut names = HashSet::new();

    for dep in deps {
        let Some(name) = dep_lookup_name(dep) else {
            continue;
        };
        match dep.registry.as_ref() {
            Some(r) if r == CRATESIO_INDEX => {
                cratesio_names.insert(name);
            }
            _ => {
                names.insert(name);
            }
        }
    }
    let cratesio_names: Vec<CanonicalName> = cratesio_names.into_iter().collect();
    let names: Vec<CanonicalName> = names.into_iter().collect();

    let mut info: HashMap<String, DepInfo> = HashMap::new();
    let mut cratesio_crates: Vec<(String, Option<String>)> = Vec::new();
    let mut cratesio_versions: Vec<(String, String)> = Vec::new();
    let mut crates: Vec<(String, Option<String>)> = Vec::new();
    let mut versions: Vec<(String, String)> = Vec::new();

    // Chunked to stay below the bind parameter limit of SQLite
    for chunk in cratesio_names.chunks(1000) {
        cratesio_crates.extend(
            cratesio_crate::Entity::find()
                .select_only()
                .columns([
                    cratesio_crate::Column::Name,
                    cratesio_crate::Column::Description,
                ])
                .filter(cratesio_crate::Column::Name.is_in(chunk.iter().cloned()))
                .into_tuple::<(String, Option<String>)>()
                .all(db_con)
                .await?,
        );
        cratesio_versions.extend(
            cratesio_index::Entity::find()
                .select_only()
                .column(cratesio_crate::Column::Name)
                .column(cratesio_index::Column::Vers)
                .inner_join(cratesio_crate::Entity)
                .filter(cratesio_crate::Column::Name.is_in(chunk.iter().cloned()))
                .filter(cratesio_index::Column::Yanked.eq(false))
                .into_tuple::<(String, String)>()
                .all(db_con)
                .await?,
        );
    }
    for chunk in names.chunks(1000) {
        crates.extend(
            krate::Entity::find()
                .select_only()
                .columns([krate::Column::Name, krate::Column::Description])
                .filter(krate::Column::Name.is_in(chunk.iter().cloned()))
                .into_tuple::<(String, Option<String>)>()
                .all(db_con)
                .await?,
        );
        versions.extend(
            crate_index::Entity::find()
                .select_only()
                .column(krate::Column::Name)
                .column(crate_index::Column::Vers)
                .inner_join(krate::Entity)
                .filter(krate::Column::Name.is_in(chunk.iter().cloned()))
                .filter(crate_index::Column::Yanked.eq(false))
                .into_tuple::<(String, String)>()
                .all(db_con)
                .await?,
        );
    }

    for (name, description) in cratesio_crates.into_iter().chain(crates) {
        info.entry(name).or_default().description = description;
    }
    let mut all_versions: HashMap<String, Vec<String>> = HashMap::new();
    for (name, version) in cratesio_versions.into_iter().chain(versions) {
        all_versions.entry(name).or_default().push(version);
    }
    for (name, versions) in all_versions {
        info.entry(name).or_default().latest_version =
            latest_version(versions.iter().map(String::as_str));
    }

    Ok(info)
}

pub async fn insert_admin_credentials<C: ConnectionTrait>(
    db_con: &C,
    con_string: &ConString,
) -> DbResult<()> {
    let hashed_pwd = hash_pwd(&con_string.admin_pwd()).await;

    let admin = user::ActiveModel {
        name: Set("admin".to_string()),
        pwd: Set(hashed_pwd),
        salt: Set(String::new()),
        is_admin: Set(true),
        is_read_only: Set(false),
        ..Default::default()
    };

    let res = user::Entity::insert(admin)
        .on_conflict_do_nothing_on([user::Column::Name])
        .exec(db_con)
        .await?;
    // Another Kellnr instance that starts at the same time created the admin
    let TryInsertResult::Inserted(res) = res else {
        return Ok(());
    };

    if let Some(token) = &con_string.admin_token() {
        let auth_token_hash = hash_token(token);

        let auth_token = auth_token::ActiveModel {
            name: Set("admin".to_string()),
            token: Set(auth_token_hash),
            user_fk: Set(res.last_insert_id),
            created: Set(Some(
                chrono::Utc::now().format(super::DB_DATE_FORMAT).to_string(),
            )),
            ..Default::default()
        };
        auth_token::Entity::insert(auth_token).exec(db_con).await?;
    }

    Ok(())
}

pub async fn no_user_exists<C: ConnectionTrait>(db_con: &C) -> DbResult<bool> {
    let id = user::Entity::find()
        .one(db_con)
        .await?
        .map(|model| model.id);

    Ok(id.is_none())
}

/// Fail with [`DbError::CrateNameConflict`] if a crate with the same canonical
/// name as `name` exists, like cargo and crates.io treat names that only
/// differ in case or in `-` versus `_` as the same crate. Used before creating
/// a new crate to prevent typosquatting.
pub async fn ensure_no_similar_crate<C: ConnectionTrait>(
    db_con: &C,
    name: &OriginalName,
) -> DbResult<()> {
    let similar = krate::Entity::find()
        .filter(krate::Column::Name.eq(name.to_canonical()))
        .one(db_con)
        .await?;

    match similar {
        Some(krate) => Err(name_conflict(name, krate.original_name)),
        None => Ok(()),
    }
}

/// Fail with [`DbError::CrateNameConflict`] if `name` is spelled with other
/// separators than the existing crate `existing_name` of the same canonical
/// name. Names that only differ in case are the same crate, they are rejected
/// later with a better error for its owners.
pub fn ensure_same_separators(name: &OriginalName, existing_name: &str) -> DbResult<()> {
    if name.to_lowercase() == existing_name.to_lowercase() {
        Ok(())
    } else {
        Err(name_conflict(name, existing_name.to_string()))
    }
}

pub fn name_conflict(name: &OriginalName, existing_name: String) -> DbError {
    DbError::CrateNameConflict(name.to_lowercase(), existing_name)
}

/// Whether the insert of a crate failed on the unique crate name, as a
/// concurrent publish created a crate with the same canonical name.
pub fn is_unique_violation(err: &DbErr) -> bool {
    matches!(err.sql_err(), Some(SqlErr::UniqueConstraintViolation(_)))
}

/// Fail with [`DbError::NotOwner`] unless `user_name` owns the crate or is an admin.
pub async fn ensure_owner_or_admin<C: ConnectionTrait>(
    db_con: &C,
    user_name: &str,
    crate_name: &str,
    crate_id: i64,
) -> DbResult<()> {
    let user = user::Entity::find()
        .filter(user::Column::Name.eq(user_name))
        .one(db_con)
        .await?
        .ok_or_else(|| DbError::UserNotFound(user_name.to_string()))?;

    if user.is_admin {
        return Ok(());
    }

    let is_owner = owner::Entity::find()
        .filter(owner::Column::CrateFk.eq(crate_id))
        .filter(owner::Column::UserFk.eq(user.id))
        .one(db_con)
        .await?
        .is_some();

    if is_owner {
        Ok(())
    } else {
        Err(DbError::NotOwner(crate_name.to_string()))
    }
}

/// Find a version of the crate that equals `version` when build metadata is
/// ignored, as cargo treats them as the same version.
pub async fn find_version_ignoring_build<C: ConnectionTrait>(
    db_con: &C,
    crate_id: i64,
    version: &Version,
) -> DbResult<Option<String>> {
    Ok(crate_meta::Entity::find()
        .filter(crate_meta::Column::CrateFk.eq(crate_id))
        .all(db_con)
        .await?
        .into_iter()
        .map(|meta| meta.version)
        .find(|existing| Version::from_unchecked_str(existing).eq_ignoring_build(version)))
}

pub async fn add_owner_if_not_exists<C: ConnectionTrait>(
    db_con: &C,
    owner_name: &str,
    crate_id: i64,
) -> DbResult<()> {
    let user_fk = user::Entity::find()
        .filter(user::Column::Name.eq(owner_name))
        .one(db_con)
        .await?
        .map(|model| model.id)
        .ok_or_else(|| DbError::UserNotFound(owner_name.to_string()))?;

    let existing_owner = owner::Entity::find()
        .filter(owner::Column::CrateFk.eq(crate_id))
        .filter(owner::Column::UserFk.eq(user_fk))
        .one(db_con)
        .await?;

    if existing_owner.is_none() {
        let o = owner::ActiveModel {
            user_fk: Set(user_fk),
            crate_fk: Set(crate_id),
            ..Default::default()
        };

        o.insert(db_con).await?;
    }
    Ok(())
}

pub async fn add_crate_index<C: ConnectionTrait>(
    db_con: &C,
    pub_metadata: &PublishMetadata,
    cksum: &str,
    crate_id: i64,
) -> DbResult<()> {
    let index_data = IndexMetadata::from_reg_meta(pub_metadata, cksum);

    let deps = if index_data.deps.is_empty() {
        None
    } else {
        let deps = serde_json::to_value(&index_data.deps)
            .map_err(|e| DbError::FailedToConvertToJson(e.to_string()))?;
        Some(deps)
    };

    // All features are stored together and split again when the index is read.
    let features = serde_json::to_value(&pub_metadata.features)
        .map_err(|e| DbError::FailedToConvertToJson(e.to_string()))?;

    let ci = crate_index::ActiveModel {
        id: ActiveValue::default(),
        name: Set(index_data.name),
        vers: Set(index_data.vers),
        deps: Set(deps),
        cksum: Set(cksum.to_owned()),
        features: Set(Some(features)),
        yanked: ActiveValue::default(),
        pubtime: Set(index_data.pubtime.map(|dt| dt.naive_utc())),
        links: Set(index_data.links),
        v: Set(index_data.v.unwrap_or(1) as i32),
        crate_fk: Set(crate_id),
        rust_version: Set(index_data.rust_version),
    };

    let ci = ci.insert(db_con).await?;

    let dependencies: Vec<crate_dependency::ActiveModel> = index_data
        .deps
        .iter()
        .filter_map(LocalDependency::from_index_dep)
        .map(|dep| crate_dependency::ActiveModel {
            id: ActiveValue::default(),
            crate_index_fk: Set(ci.id),
            name: Set(dep.name.into_inner()),
            version_req: Set(dep.version_req),
            kind: Set(dep.kind),
        })
        .collect();
    if !dependencies.is_empty() {
        crate_dependency::Entity::insert_many(dependencies)
            .exec(db_con)
            .await?;
    }
    Ok(())
}

pub async fn add_crate_metadata<C: ConnectionTrait>(
    db_con: &C,
    pub_metadata: &PublishMetadata,
    size: i64,
    created: &str,
    crate_id: i64,
    publisher: Option<&str>,
    publisher_token: Option<&str>,
) -> DbResult<()> {
    let cm = crate_meta::ActiveModel {
        id: ActiveValue::default(),
        version: Set(pub_metadata.vers.clone()),
        created: Set(created.to_string()),
        downloads: Set(0),
        crate_fk: Set(crate_id),
        readme: Set(pub_metadata.readme.clone()),
        license: Set(pub_metadata.license.clone()),
        license_file: Set(pub_metadata.license_file.clone()),
        documentation: Set(pub_metadata.documentation.clone()),
        size: Set(size),
        publisher: Set(publisher.map(str::to_string)),
        publisher_token: Set(publisher_token.map(str::to_string)),
    };

    cm.insert(db_con).await?;

    Ok(())
}

/// The values without duplicates, in the order they first occur.
fn unique_values(values: &[String]) -> Vec<&String> {
    let mut seen = HashSet::new();
    values.iter().filter(|v| seen.insert(*v)).collect()
}

pub async fn update_crate_categories<C: ConnectionTrait>(
    db_con: &C,
    pub_metadata: &PublishMetadata,
    crate_id: i64,
) -> DbResult<()> {
    // Delete all existing categories relationships as only the latest list of categories is relevant
    crate_category_to_crate::Entity::delete_many()
        .filter(crate_category_to_crate::Column::CrateFk.eq(crate_id))
        .exec(db_con)
        .await?;

    // Set the latest list of categories for the crate. Categories are shared
    // between crates, so a concurrent publish may create the same one.
    for category in unique_values(&pub_metadata.categories) {
        crate_category::Entity::insert(crate_category::ActiveModel {
            id: ActiveValue::default(),
            category: Set(category.clone()),
        })
        .on_conflict_do_nothing_on([crate_category::Column::Category])
        .exec_without_returning(db_con)
        .await?;
        let category_fk = crate_category::Entity::find()
            .filter(crate_category::Column::Category.eq(category))
            .one(db_con)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(format!("crate category {category}")))?
            .id;

        crate_category_to_crate::Entity::insert(crate_category_to_crate::ActiveModel {
            id: ActiveValue::default(),
            crate_fk: Set(crate_id),
            category_fk: Set(category_fk),
        })
        .on_conflict_do_nothing_on([
            crate_category_to_crate::Column::CrateFk,
            crate_category_to_crate::Column::CategoryFk,
        ])
        .exec_without_returning(db_con)
        .await?;
    }

    Ok(())
}

pub async fn update_crate_keywords<C: ConnectionTrait>(
    db_con: &C,
    pub_metadata: &PublishMetadata,
    crate_id: i64,
) -> DbResult<()> {
    // Delete all existing keywords relationships as only the latest list of keywords is relevant
    crate_keyword_to_crate::Entity::delete_many()
        .filter(crate_keyword_to_crate::Column::CrateFk.eq(crate_id))
        .exec(db_con)
        .await?;

    // Set the latest list of keywords for the crate. Keywords are shared
    // between crates, so a concurrent publish may create the same one.
    for keyword in unique_values(&pub_metadata.keywords) {
        crate_keyword::Entity::insert(crate_keyword::ActiveModel {
            id: ActiveValue::default(),
            keyword: Set(keyword.clone()),
        })
        .on_conflict_do_nothing_on([crate_keyword::Column::Keyword])
        .exec_without_returning(db_con)
        .await?;
        let keyword_fk = crate_keyword::Entity::find()
            .filter(crate_keyword::Column::Keyword.eq(keyword))
            .one(db_con)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(format!("crate keyword {keyword}")))?
            .id;

        crate_keyword_to_crate::Entity::insert(crate_keyword_to_crate::ActiveModel {
            id: ActiveValue::default(),
            crate_fk: Set(crate_id),
            keyword_fk: Set(keyword_fk),
        })
        .on_conflict_do_nothing_on([
            crate_keyword_to_crate::Column::CrateFk,
            crate_keyword_to_crate::Column::KeywordFk,
        ])
        .exec_without_returning(db_con)
        .await?;
    }

    Ok(())
}

pub async fn update_crate_authors<C: ConnectionTrait>(
    db_con: &C,
    pub_metadata: &PublishMetadata,
    crate_id: i64,
) -> DbResult<()> {
    let authors = pub_metadata.authors.clone().unwrap_or_default();

    // Delete all existing authors relationships as only the latest list of authors is relevant
    crate_author_to_crate::Entity::delete_many()
        .filter(crate_author_to_crate::Column::CrateFk.eq(crate_id))
        .exec(db_con)
        .await?;

    // Set the latest list of authors for the crate. Authors are shared
    // between crates, so a concurrent publish may create the same one.
    for author in unique_values(&authors) {
        crate_author::Entity::insert(crate_author::ActiveModel {
            id: ActiveValue::default(),
            author: Set(author.clone()),
        })
        .on_conflict_do_nothing_on([crate_author::Column::Author])
        .exec_without_returning(db_con)
        .await?;
        let author_fk = crate_author::Entity::find()
            .filter(crate_author::Column::Author.eq(author))
            .one(db_con)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(format!("crate author {author}")))?
            .id;

        crate_author_to_crate::Entity::insert(crate_author_to_crate::ActiveModel {
            id: ActiveValue::default(),
            crate_fk: Set(crate_id),
            author_fk: Set(author_fk),
        })
        .on_conflict_do_nothing_on([
            crate_author_to_crate::Column::CrateFk,
            crate_author_to_crate::Column::AuthorFk,
        ])
        .exec_without_returning(db_con)
        .await?;
    }

    Ok(())
}

/// The index entries of all versions of a crate, in the order they were
/// published, so the index file and its `ETag` are always the same.
pub async fn crate_indices<C: ConnectionTrait>(
    db_con: &C,
    crate_id: i64,
) -> DbResult<Vec<crate_index::Model>> {
    Ok(crate_index::Entity::find()
        .filter(crate_index::Column::CrateFk.eq(crate_id))
        .order_by_asc(crate_index::Column::Id)
        .all(db_con)
        .await?)
}

pub async fn compute_etag<C: ConnectionTrait>(
    db_con: &C,
    crate_name: &str,
    crate_id: i64,
) -> DbResult<String> {
    let crate_indices = crate_indices(db_con, crate_id).await?;
    let index_metadata = crate_index_model_to_index_metadata(crate_name, crate_indices)?;
    let data = index_metadata_to_bytes(&index_metadata)?;

    Ok(sha256::digest(data))
}

pub fn index_metadata_to_bytes(index_metadata: &[IndexMetadata]) -> DbResult<Vec<u8>> {
    IndexMetadata::serialize_indices(index_metadata)
        .map(String::into_bytes)
        .map_err(|e| DbError::FailedToConvertToJson(format!("{e}")))
}

pub fn crate_index_model_to_index_metadata(
    crate_name: &str,
    crate_indices: Vec<crate_index::Model>,
) -> DbResult<Vec<IndexMetadata>> {
    let mut index_metadata = vec![];
    for ci in crate_indices {
        let deps = match ci.deps {
            Some(deps) => serde_json::value::from_value(deps).map_err(|e| {
                DbError::FailedToConvertFromJson(format!(
                    "Failed to deserialize crate dependencies of {crate_name}: {e}"
                ))
            })?,
            None => vec![],
        };
        let features = ci.features.clone().unwrap_or_default();
        let features = serde_json::value::from_value(features).map_err(|e| {
            DbError::FailedToConvertFromJson(format!(
                "Failed to deserialize crate features of {crate_name}: {e}"
            ))
        })?;
        // Older versions of Kellnr stored v1 entries that use the newer
        // feature syntax, so the version is raised if needed.
        let (features, features2) = split_features(features);
        let v = u32::try_from(ci.v)
            .ok()
            .map(|v| Ord::max(v, schema_version(features2.as_ref())));

        let cm = IndexMetadata {
            name: ci.name,
            vers: ci.vers,
            deps,
            cksum: ci.cksum,
            features,
            yanked: ci.yanked,
            pubtime: ci.pubtime.map(|dt| dt.and_utc()),
            links: ci.links,
            v,
            features2,
            rust_version: ci.rust_version,
        };
        index_metadata.push(cm);
    }
    Ok(index_metadata)
}

pub fn cratesio_index_model_to_index_metadata(
    crate_name: &str,
    crate_indices: Vec<cratesio_index::Model>,
) -> DbResult<Vec<IndexMetadata>> {
    let mut index_metadata = vec![];
    for ci in crate_indices {
        let deps = match ci.deps {
            Some(deps) => serde_json::value::from_value(deps).map_err(|e| {
                DbError::FailedToConvertFromJson(format!(
                    "Failed to deserialize crate dependencies of {crate_name}: {e}"
                ))
            })?,
            None => vec![],
        };
        let features = ci.features.clone().unwrap_or_default();
        let features = serde_json::value::from_value(features).map_err(|e| {
            DbError::FailedToConvertFromJson(format!(
                "Failed to deserialize crate features of {crate_name}: {e}"
            ))
        })?;

        let features2 = ci.features2.clone().unwrap_or_default();
        let features2 = serde_json::value::from_value(features2).map_err(|e| {
            DbError::FailedToConvertFromJson(format!(
                "Failed to deserialize crate features of {crate_name}: {e}"
            ))
        })?;

        let cm = IndexMetadata {
            name: ci.name,
            vers: ci.vers.clone(),
            deps,
            cksum: ci.cksum.clone(),
            pubtime: ci.pubtime.map(|dt| dt.and_utc()),
            features,
            features2,
            yanked: ci.yanked,
            links: ci.links.clone(),
            v: u32::try_from(ci.v).ok(),
            rust_version: ci.rust_version.clone(),
        };
        index_metadata.push(cm);
    }
    Ok(index_metadata)
}

pub async fn update_etag<C: ConnectionTrait>(
    db_con: &C,
    crate_name: &str,
    crate_id: i64,
) -> DbResult<()> {
    let etag = compute_etag(db_con, crate_name, crate_id).await?;
    let mut krate: krate::ActiveModel = krate::Entity::find()
        .filter(krate::Column::Id.eq(crate_id))
        .one(db_con)
        .await?
        .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?
        .into();
    krate.e_tag = Set(etag);
    krate.update(db_con).await?;
    Ok(())
}

pub async fn get_max_version_from_id<C: ConnectionTrait>(
    db_con: &C,
    crate_id: i64,
) -> DbResult<Version> {
    let krate = krate::Entity::find_by_id(crate_id).one(db_con).await?;

    let k = krate.ok_or(DbError::FailedToGetMaxVersionById(crate_id))?;
    let v = Version::try_from(&k.max_version)
        .map_err(|_| DbError::FailedToGetMaxVersionById(crate_id))?;
    Ok(v)
}
