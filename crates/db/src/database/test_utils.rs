//! Test utilities for database testing.
//!
//! These functions provide convenient ways to set up test data
//! in the database for integration and unit tests.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::original_name::OriginalName;
use kellnr_common::prefetch::Prefetch;
use kellnr_common::publish_metadata::PublishMetadata;
use kellnr_common::version::Version;
use kellnr_entity::{crate_index, crate_meta, cratesio_crate, krate, session, user};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, ExprTrait, QueryFilter, Set,
};

use super::{DB_DATE_FORMAT, Database};
use crate::CrateMeta;
use crate::error::DbError;
use crate::provider::{DbProvider, DbResult};

/// Add a cached crate with a specified download count.
pub async fn test_add_cached_crate_with_downloads(
    db: &Database,
    name: &str,
    version: &str,
    downloads: u64,
) -> DbResult<()> {
    let _ = test_add_cached_crate(db, name, version).await?;

    let krate = cratesio_crate::Entity::find()
        .filter(cratesio_crate::Column::Name.eq(name))
        .one(&db.db_con)
        .await?
        .ok_or(DbError::CrateNotFound(name.to_string()))?;

    let total_downloads = krate.total_downloads as u64;

    let mut krate: cratesio_crate::ActiveModel = krate.into();
    krate.total_downloads = Set((total_downloads + downloads) as i64);
    krate.update(&db.db_con).await?;

    Ok(())
}

/// Add a cached crate for testing.
pub async fn test_add_cached_crate(db: &Database, name: &str, version: &str) -> DbResult<Prefetch> {
    let etag = "etag";
    let last_modified = "last_modified";
    let description = Some(String::from("description"));
    let indices = vec![IndexMetadata {
        name: name.to_string(),
        vers: version.to_string(),
        deps: vec![],
        cksum: "cksum".to_string(),
        features: BTreeMap::new(),
        features2: None,
        pubtime: None,
        yanked: false,
        links: None,
        v: Some(1),
    }];

    db.add_cratesio_prefetch_data(
        &OriginalName::from_unchecked(name.to_string()),
        etag,
        last_modified,
        description,
        &indices,
    )
    .await
}

/// Add a test crate.
pub async fn test_add_crate(
    db: &Database,
    name: &str,
    owner: &str,
    version: &Version,
    created: &DateTime<Utc>,
) -> DbResult<i64> {
    let pm = PublishMetadata {
        name: name.to_string(),
        vers: version.to_string(),
        ..PublishMetadata::default()
    };
    let user = user::Entity::find()
        .filter(user::Column::Name.eq(owner))
        .one(&db.db_con)
        .await?;
    if user.is_none() {
        db.add_user(name, "pwd", "salt", false, false).await?;
    }

    db.add_crate(&pm, "cksum", created, owner).await
}

/// Add a test crate with a specified download count.
pub async fn test_add_crate_with_downloads(
    db: &Database,
    name: &str,
    owner: &str,
    version: &Version,
    created: &DateTime<Utc>,
    downloads: Option<i64>,
) -> DbResult<i64> {
    let pm = PublishMetadata {
        name: name.to_string(),
        vers: version.to_string(),
        ..PublishMetadata::default()
    };
    let user = user::Entity::find()
        .filter(user::Column::Name.eq(owner))
        .one(&db.db_con)
        .await?;
    if user.is_none() {
        db.add_user(name, "pwd", "salt", false, false).await?;
    }

    db.add_crate(&pm, "cksum", created, owner).await?;
    let (cm, krate) = crate_meta::Entity::find()
        .find_also_related(krate::Entity)
        .filter(krate::Column::Name.eq(name))
        .filter(crate_meta::Column::Version.eq(version.to_string()))
        .one(&db.db_con)
        .await?
        .ok_or(DbError::CrateNotFound(name.to_string()))?;
    let mut cm: crate_meta::ActiveModel = cm.into();

    let current_downloads = krate.as_ref().unwrap().total_downloads;
    let crate_id = krate.as_ref().unwrap().id;

    let mut krate: krate::ActiveModel = krate.unwrap().into();
    krate.total_downloads = Set(current_downloads + downloads.unwrap_or(0));
    krate.update(&db.db_con).await?;
    cm.downloads = Set(downloads.unwrap_or_default());
    cm.update(&db.db_con).await?;
    Ok(crate_id)
}

/// Add test crate metadata.
pub async fn test_add_crate_meta(
    db: &Database,
    crate_id: i64,
    version: &str,
    created: &DateTime<Utc>,
    downloads: Option<i64>,
) -> DbResult<()> {
    let cm = crate_meta::ActiveModel {
        id: ActiveValue::default(),
        version: Set(version.to_string()),
        created: Set(created.to_string()),
        downloads: Set(downloads.unwrap_or_default()),
        crate_fk: Set(crate_id),
        ..Default::default()
    };

    cm.insert(&db.db_con).await?;

    Ok(())
}

/// Delete crate index entries for testing.
pub async fn test_delete_crate_index(db: &Database, crate_id: i64) -> DbResult<()> {
    crate_index::Entity::delete_many()
        .filter(crate_index::Column::CrateFk.eq(crate_id))
        .exec(&db.db_con)
        .await?;
    Ok(())
}

/// Clean database by removing old sessions.
pub async fn clean_db(db: &Database, session_age: std::time::Duration) -> DbResult<()> {
    let session_age = chrono::Duration::from_std(session_age).unwrap();
    let now = std::ops::Add::add(Utc::now(), session_age)
        .format(DB_DATE_FORMAT)
        .to_string();

    session::Entity::delete_many()
        .filter(Expr::col(session::Column::Created).lt(now))
        .exec(&db.db_con)
        .await?;

    Ok(())
}

/// Get crate meta list by crate ID.
pub async fn get_crate_meta_list(db: &Database, crate_id: i64) -> DbResult<Vec<CrateMeta>> {
    let cm: Vec<(crate_meta::Model, Option<krate::Model>)> = crate_meta::Entity::find()
        .find_also_related(krate::Entity)
        .filter(crate_meta::Column::CrateFk.eq(crate_id))
        .all(&db.db_con)
        .await?;

    let crate_metas: Vec<CrateMeta> = cm
        .into_iter()
        .map(|(m, c)| CrateMeta {
            name: c.unwrap().name, // Unwarp is ok, as a relation always exists
            id: m.id,
            version: m.version,
            created: m.created,
            downloads: m.downloads,
            crate_fk: m.crate_fk,
        })
        .collect();

    Ok(crate_metas)
}
