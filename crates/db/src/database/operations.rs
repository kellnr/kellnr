//! Core database operations.
//!
//! Standalone async functions that take a database connection and perform
//! specific operations. Used by the Database impl to keep the main module
//! focused on the `DbProvider` trait implementation.

use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::publish_metadata::PublishMetadata;
use kellnr_entity::{
    auth_token, crate_author, crate_author_to_crate, crate_category, crate_category_to_crate,
    crate_index, crate_keyword, crate_keyword_to_crate, crate_meta, cratesio_crate, cratesio_index,
    krate, owner, user,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set,
};

use crate::ConString;
use crate::error::DbError;
use crate::password::{hash_pwd, hash_token};
use crate::provider::DbResult;

pub async fn get_desc_for_crate_dep<C: ConnectionTrait>(
    db_con: &C,
    name: &str,
    registry: Option<&str>,
) -> DbResult<Option<String>> {
    let desc = if registry.unwrap_or_default() == "https://github.com/rust-lang/crates.io-index" {
        let krate = cratesio_crate::Entity::find()
            .filter(cratesio_crate::Column::Name.eq(name))
            .one(db_con)
            .await?;
        krate.and_then(|krate| krate.description)
    } else {
        // Not a crates.io dependency.
        // We cannot know that the crate is from this kellnr instance, but we give it a try.
        let krate = krate::Entity::find()
            .filter(krate::Column::Name.eq(name))
            .one(db_con)
            .await?;
        krate.and_then(|krate| krate.description)
    };

    Ok(desc)
}

pub async fn insert_admin_credentials<C: ConnectionTrait>(
    db_con: &C,
    con_string: &ConString,
) -> DbResult<()> {
    let hashed_pwd = hash_pwd(&con_string.admin_pwd(), &con_string.salt());

    let admin = user::ActiveModel {
        name: Set("admin".to_string()),
        pwd: Set(hashed_pwd),
        salt: Set(con_string.salt()),
        is_admin: Set(true),
        is_read_only: Set(false),
        ..Default::default()
    };

    let res: sea_orm::InsertResult<user::ActiveModel> =
        user::Entity::insert(admin).exec(db_con).await?;
    let auth_token_hash = hash_token(&con_string.admin_token());

    let auth_token = auth_token::ActiveModel {
        name: Set("admin".to_string()),
        token: Set(auth_token_hash),
        user_fk: Set(res.last_insert_id),
        ..Default::default()
    };
    auth_token::Entity::insert(auth_token).exec(db_con).await?;

    Ok(())
}

pub async fn no_user_exists<C: ConnectionTrait>(db_con: &C) -> DbResult<bool> {
    let id = user::Entity::find()
        .one(db_con)
        .await?
        .map(|model| model.id);

    Ok(id.is_none())
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

    let features = serde_json::to_value(&index_data.features)
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
    };

    ci.insert(db_con).await?;
    Ok(())
}

pub async fn add_crate_metadata<C: ConnectionTrait>(
    db_con: &C,
    pub_metadata: &PublishMetadata,
    created: &str,
    crate_id: i64,
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
    };

    cm.insert(db_con).await?;

    Ok(())
}

pub async fn update_crate_categories<C: ConnectionTrait>(
    db_con: &C,
    pub_metadata: &PublishMetadata,
    crate_id: i64,
) -> DbResult<()> {
    let categories = pub_metadata.categories.clone();

    // Delete all existing categories relationships as only the latest list of categories is relevant
    crate_category_to_crate::Entity::delete_many()
        .filter(crate_category_to_crate::Column::CrateFk.eq(crate_id))
        .exec(db_con)
        .await?;

    // Set the latest list of categories for the crate
    for category in categories {
        let category_fk = crate_category::Entity::find()
            .filter(crate_category::Column::Category.eq(category.clone()))
            .one(db_con)
            .await?
            .map(|model| model.id);

        // If the category does not exist, create it
        let category_fk = if let Some(category_fk) = category_fk {
            category_fk
        } else {
            let cc = crate_category::ActiveModel {
                id: ActiveValue::default(),
                category: Set(category.clone()),
            };

            cc.insert(db_con).await?.id
        };

        // Add the relationship between the crate and the category
        let cctc = crate_category_to_crate::ActiveModel {
            id: ActiveValue::default(),
            crate_fk: Set(crate_id),
            category_fk: Set(category_fk),
        };
        cctc.insert(db_con).await?;
    }

    Ok(())
}

pub async fn update_crate_keywords<C: ConnectionTrait>(
    db_con: &C,
    pub_metadata: &PublishMetadata,
    crate_id: i64,
) -> DbResult<()> {
    let keywords = pub_metadata.keywords.clone();

    // Delete all existing keywords relationships as only the latest list of keywords is relevant
    crate_keyword_to_crate::Entity::delete_many()
        .filter(crate_keyword_to_crate::Column::CrateFk.eq(crate_id))
        .exec(db_con)
        .await?;

    // Set the latest list of keywords for the crate
    for keyword in keywords {
        let keyword_fk = crate_keyword::Entity::find()
            .filter(crate_keyword::Column::Keyword.eq(keyword.clone()))
            .one(db_con)
            .await?
            .map(|model| model.id);

        // If the keyword does not exist, create it
        let keyword_fk = if let Some(keyword_fk) = keyword_fk {
            keyword_fk
        } else {
            let ck = crate_keyword::ActiveModel {
                id: ActiveValue::default(),
                keyword: Set(keyword.clone()),
            };

            ck.insert(db_con).await?.id
        };

        // Add the relationship between the crate and the keyword
        let cktc = crate_keyword_to_crate::ActiveModel {
            id: ActiveValue::default(),
            crate_fk: Set(crate_id),
            keyword_fk: Set(keyword_fk),
        };
        cktc.insert(db_con).await?;
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

    // Set the latest list of authors for the crate
    for author in authors {
        let author_fk = crate_author::Entity::find()
            .filter(crate_author::Column::Author.eq(author.clone()))
            .one(db_con)
            .await?
            .map(|model| model.id);

        // If the author does not exist, create it
        let author_fk = if let Some(author_fk) = author_fk {
            author_fk
        } else {
            let ca = crate_author::ActiveModel {
                id: ActiveValue::default(),
                author: Set(author.clone()),
            };

            ca.insert(db_con).await?.id
        };

        // Add the relationship between the crate and the author
        let catc = crate_author_to_crate::ActiveModel {
            id: ActiveValue::default(),
            crate_fk: Set(crate_id),
            author_fk: Set(author_fk),
        };
        catc.insert(db_con).await?;
    }

    Ok(())
}

pub async fn compute_etag<C: ConnectionTrait>(
    db_con: &C,
    crate_name: &str,
    crate_id: i64,
) -> DbResult<String> {
    let crate_indices = crate_index::Entity::find()
        .filter(crate_index::Column::CrateFk.eq(crate_id))
        .all(db_con)
        .await?;

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
            Some(ref deps) => serde_json::value::from_value(deps.to_owned()).map_err(|e| {
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

        let cm = IndexMetadata {
            name: ci.name,
            vers: ci.vers,
            deps,
            cksum: ci.cksum,
            features,
            yanked: ci.yanked,
            pubtime: ci.pubtime.map(|dt| dt.and_utc()),
            links: ci.links,
            v: Some(ci.v as u32),
            features2: None,
        };
        index_metadata.push(cm);
    }
    Ok(index_metadata)
}

pub fn cratesio_index_model_to_index_metadata(
    crate_name: &NormalizedName,
    crate_indices: Vec<cratesio_index::Model>,
) -> DbResult<Vec<IndexMetadata>> {
    let mut index_metadata = vec![];
    for ci in crate_indices {
        let deps = match ci.deps {
            Some(ref deps) => serde_json::value::from_value(deps.to_owned()).map_err(|e| {
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
            v: Some(ci.v as u32),
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
    let krate = krate::Entity::find()
        .filter(krate::Column::Id.eq(crate_id))
        .one(db_con)
        .await?
        .ok_or(DbError::CrateNotFound(crate_name.to_string()))?;
    let mut krate: krate::ActiveModel = krate.into();
    krate.e_tag = Set(etag);
    krate.update(db_con).await?;
    Ok(())
}

pub async fn get_max_version_from_id<C: ConnectionTrait>(
    db_con: &C,
    crate_id: i64,
) -> DbResult<kellnr_common::version::Version> {
    let krate = krate::Entity::find_by_id(crate_id).one(db_con).await?;

    let k = krate.ok_or(DbError::FailedToGetMaxVersionById(crate_id))?;
    let v = kellnr_common::version::Version::try_from(&k.max_version)
        .map_err(|_| DbError::FailedToGetMaxVersionById(crate_id))?;
    Ok(v)
}
