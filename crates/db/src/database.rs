use crate::password::{generate_salt, hash_pwd};
use crate::provider::{DbResult, PrefetchState};
use crate::tables::init_database;
use crate::{error::DbError, AuthToken, CrateMeta, CrateSummary, DbProvider, User};
use crate::{ConString, DocQueueEntry};
use chrono::{DateTime, Utc};
use common::crate_data::{CrateData, CrateRegistryDep, CrateVersionData};
use common::crate_overview::CrateOverview;
use common::cratesio_prefetch_msg::{CratesioPrefetchMsg, UpdateData};
use common::index_metadata::{IndexDep, IndexMetadata};
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::prefetch::Prefetch;
use common::publish_metadata::PublishMetadata;
use common::version::Version;
use entity::{
    auth_token, crate_author, crate_author_to_crate, crate_category, crate_category_to_crate,
    crate_index, crate_keyword, crate_keyword_to_crate, crate_meta, crate_user, cratesio_crate,
    cratesio_index, cratesio_meta, doc_queue, krate, owner, prelude::*, session, user,
};
use migration::iden::{AuthTokenIden, CrateIden, CrateMetaIden, CratesIoIden, CratesIoMetaIden};
use sea_orm::sea_query::{Alias, Expr, Query, *};
use sea_orm::{
    prelude::async_trait::async_trait, query::*, ActiveModelTrait, ColumnTrait, ConnectionTrait,
    DatabaseConnection, EntityTrait, FromQueryResult, InsertResult, ModelTrait, QueryFilter,
    RelationTrait, Set,
};
use std::collections::BTreeMap;
use std::ops::Add;
use std::path::Path;
use std::vec;

const DB_DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub struct Database {
    db_con: DatabaseConnection,
}

impl Database {
    pub fn existing(db_con: DatabaseConnection) -> Self {
        Self { db_con }
    }

    pub async fn new(con: &ConString) -> Result<Self, DbError> {
        let db_con = init_database(con)
            .await
            .map_err(|e| DbError::InitializationError(e.to_string()))?;

        if Self::no_user_exists(&db_con).await? {
            Self::insert_admin_credentials(&db_con, con).await?;
        }

        Ok(Self { db_con })
    }

    async fn get_desc_for_crate_dep(
        &self,
        name: &str,
        registry: &Option<String>,
    ) -> DbResult<Option<String>> {
        let desc = if registry == &Some("https://github.com/rust-lang/crates.io-index".to_string())
        {
            let krate = cratesio_crate::Entity::find()
                .filter(cratesio_crate::Column::Name.eq(name))
                .one(&self.db_con)
                .await?;
            krate.and_then(|krate| krate.description)
        } else {
            // Not a crates.io dependency.
            // We cannot know that the crate is from this kellnr instance, but we give it a try.
            let krate = krate::Entity::find()
                .filter(krate::Column::Name.eq(name))
                .one(&self.db_con)
                .await?;
            krate.and_then(|krate| krate.description)
        };

        Ok(desc)
    }

    async fn insert_admin_credentials(
        db_con: &DatabaseConnection,
        con_string: &ConString,
    ) -> DbResult<()> {
        let hashed_pwd = hash_pwd(&con_string.admin_pwd(), &con_string.salt());

        let admin = user::ActiveModel {
            name: Set("admin".to_string()),
            pwd: Set(hashed_pwd),
            salt: Set(con_string.salt()),
            is_admin: Set(true),
            ..Default::default()
        };

        let res: InsertResult<user::ActiveModel> = user::Entity::insert(admin).exec(db_con).await?;

        let auth_token = auth_token::ActiveModel {
            name: Set("admin".to_string()),
            token: Set(con_string.admin_token()),
            user_fk: Set(res.last_insert_id),
            ..Default::default()
        };
        auth_token::Entity::insert(auth_token).exec(db_con).await?;

        Ok(())
    }

    async fn no_user_exists(db_con: &DatabaseConnection) -> DbResult<bool> {
        let id = user::Entity::find()
            .one(db_con)
            .await?
            .map(|model| model.id);

        Ok(id.is_none())
    }

    pub async fn get_crate_meta_list(&self, crate_id: i64) -> DbResult<Vec<CrateMeta>> {
        let cm: Vec<(crate_meta::Model, Option<krate::Model>)> = crate_meta::Entity::find()
            .find_also_related(krate::Entity)
            .filter(crate_meta::Column::CrateFk.eq(crate_id))
            .all(&self.db_con)
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

    pub async fn clean_db(&self, session_age: std::time::Duration) -> DbResult<()> {
        let session_age = chrono::Duration::from_std(session_age).unwrap();
        let now = Utc::now()
            .add(session_age)
            .format(DB_DATE_FORMAT)
            .to_string();

        session::Entity::delete_many()
            .filter(Expr::col(session::Column::Created).lt(now))
            .exec(&self.db_con)
            .await?;

        Ok(())
    }

    async fn add_owner_if_not_exists(&self, owner: &str, crate_id: i64) -> DbResult<()> {
        let user_fk = user::Entity::find()
            .filter(user::Column::Name.eq(owner))
            .one(&self.db_con)
            .await?
            .map(|model| model.id)
            .ok_or_else(|| DbError::UserNotFound(owner.to_string()))?;

        let owner = owner::Entity::find()
            .filter(owner::Column::CrateFk.eq(crate_id))
            .filter(owner::Column::UserFk.eq(user_fk))
            .one(&self.db_con)
            .await?;

        if owner.is_none() {
            let o = owner::ActiveModel {
                user_fk: Set(user_fk),
                crate_fk: Set(crate_id),
                ..Default::default()
            };

            o.insert(&self.db_con).await?;
        }
        Ok(())
    }

    pub async fn test_add_cached_crate_with_downloads(
        &self,
        name: &str,
        version: &str,
        downloads: u64,
    ) -> DbResult<()> {
        let _ = self.test_add_cached_crate(name, version).await?;

        let krate = cratesio_crate::Entity::find()
            .filter(cratesio_crate::Column::Name.eq(name))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::CrateNotFound(name.to_string()))?;

        let total_downloads = krate.total_downloads as u64;

        let mut krate: cratesio_crate::ActiveModel = krate.into();
        krate.total_downloads = Set((total_downloads + downloads) as i64);
        krate.update(&self.db_con).await?;

        Ok(())
    }

    pub async fn test_add_cached_crate(&self, name: &str, version: &str) -> DbResult<Prefetch> {
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
            yanked: false,
            links: None,
            v: Some(1),
        }];

        self.add_cratesio_prefetch_data(
            &OriginalName::from_unchecked_str(name.to_string()),
            etag,
            last_modified,
            description,
            &indices,
        )
        .await
    }

    pub async fn test_add_crate(
        &self,
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
            .one(&self.db_con)
            .await?;
        if user.is_none() {
            self.add_user(name, "pwd", "salt", false).await?;
        }

        self.add_crate(&pm, "cksum", created, owner).await
    }

    pub async fn test_add_crate_with_downloads(
        &self,
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
            .one(&self.db_con)
            .await?;
        if user.is_none() {
            self.add_user(name, "pwd", "salt", false).await?;
        }

        self.add_crate(&pm, "cksum", created, owner).await?;
        let (cm, krate) = crate_meta::Entity::find()
            .find_also_related(krate::Entity)
            .filter(krate::Column::Name.eq(name))
            .filter(crate_meta::Column::Version.eq(version.to_string()))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::CrateNotFound(name.to_string()))?;
        let mut cm: crate_meta::ActiveModel = cm.into();

        let current_downloads = krate.as_ref().unwrap().total_downloads;
        let crate_id = krate.as_ref().unwrap().id;

        let mut krate: krate::ActiveModel = krate.unwrap().into();
        krate.total_downloads = Set(current_downloads + downloads.unwrap_or(0));
        krate.update(&self.db_con).await?;
        cm.downloads = Set(downloads.unwrap_or_default());
        cm.update(&self.db_con).await?;
        Ok(crate_id)
    }

    pub async fn test_add_crate_meta(
        &self,
        crate_id: i64,
        version: &str,
        created: &DateTime<Utc>,
        downloads: Option<i64>,
    ) -> DbResult<()> {
        let cm = crate_meta::ActiveModel {
            id: Default::default(),
            version: Set(version.to_string()),
            created: Set(created.to_string()),
            downloads: Set(downloads.unwrap_or_default()),
            crate_fk: Set(crate_id),
            ..Default::default()
        };

        cm.insert(&self.db_con).await?;

        Ok(())
    }

    async fn add_crate_index(
        &self,
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
            id: Default::default(),
            name: Set(index_data.name),
            vers: Set(index_data.vers),
            deps: Set(deps),
            cksum: Set(cksum.to_owned()),
            features: Set(Some(features)),
            yanked: Default::default(),
            links: Set(index_data.links),
            v: Set(index_data.v.unwrap_or(1) as i32),
            crate_fk: Set(crate_id),
        };

        ci.insert(&self.db_con).await?;
        Ok(())
    }

    async fn update_crate_categories(
        &self,
        pub_metadata: &PublishMetadata,
        crate_id: i64,
    ) -> DbResult<()> {
        let categories = pub_metadata.categories.clone();

        // Delete all existing categories relationships as only the latest list of categories is relevant
        crate_category_to_crate::Entity::delete_many()
            .filter(crate_category_to_crate::Column::CrateFk.eq(crate_id))
            .exec(&self.db_con)
            .await?;

        // Set the latest list of categories for the crate
        for category in categories {
            let category_fk = crate_category::Entity::find()
                .filter(crate_category::Column::Category.eq(category.clone()))
                .one(&self.db_con)
                .await?
                .map(|model| model.id);

            // If the category does not exist, create it
            let category_fk = match category_fk {
                Some(category_fk) => category_fk,
                None => {
                    let cc = crate_category::ActiveModel {
                        id: Default::default(),
                        category: Set(category.clone()),
                    };

                    cc.insert(&self.db_con).await?.id
                }
            };

            // Add the relationship between the crate and the category
            let cctc = crate_category_to_crate::ActiveModel {
                id: Default::default(),
                crate_fk: Set(crate_id),
                category_fk: Set(category_fk),
            };
            cctc.insert(&self.db_con).await?;
        }

        Ok(())
    }

    async fn update_crate_keywords(
        &self,
        pub_metadata: &PublishMetadata,
        crate_id: i64,
    ) -> DbResult<()> {
        let keywords = pub_metadata.keywords.clone();

        // Delete all existing keywords relationships as only the latest list of keywords is relevant
        crate_keyword_to_crate::Entity::delete_many()
            .filter(crate_keyword_to_crate::Column::CrateFk.eq(crate_id))
            .exec(&self.db_con)
            .await?;

        // Set the latest list of keywords for the crate
        for keyword in keywords {
            let keyword_fk = crate_keyword::Entity::find()
                .filter(crate_keyword::Column::Keyword.eq(keyword.clone()))
                .one(&self.db_con)
                .await?
                .map(|model| model.id);

            // If the keyword does not exist, create it
            let keyword_fk = match keyword_fk {
                Some(keyword_fk) => keyword_fk,
                None => {
                    let ck = crate_keyword::ActiveModel {
                        id: Default::default(),
                        keyword: Set(keyword.clone()),
                    };

                    ck.insert(&self.db_con).await?.id
                }
            };

            // Add the relationship between the crate and the keyword
            let cktc = crate_keyword_to_crate::ActiveModel {
                id: Default::default(),
                crate_fk: Set(crate_id),
                keyword_fk: Set(keyword_fk),
            };
            cktc.insert(&self.db_con).await?;
        }

        Ok(())
    }

    async fn update_crate_authors(
        &self,
        pub_metadata: &PublishMetadata,
        crate_id: i64,
    ) -> DbResult<()> {
        let authors = pub_metadata.authors.clone().unwrap_or_default();

        // Delete all existing authors relationships as only the latest list of authors is relevant
        crate_author_to_crate::Entity::delete_many()
            .filter(crate_author_to_crate::Column::CrateFk.eq(crate_id))
            .exec(&self.db_con)
            .await?;

        // Set the latest list of authors for the crate
        for author in authors {
            let author_fk = crate_author::Entity::find()
                .filter(crate_author::Column::Author.eq(author.clone()))
                .one(&self.db_con)
                .await?
                .map(|model| model.id);

            // If the author does not exist, create it
            let author_fk = match author_fk {
                Some(author_fk) => author_fk,
                None => {
                    let ca = crate_author::ActiveModel {
                        id: Default::default(),
                        author: Set(author.clone()),
                    };

                    ca.insert(&self.db_con).await?.id
                }
            };

            // Add the relationship between the crate and the author
            let catc = crate_author_to_crate::ActiveModel {
                id: Default::default(),
                crate_fk: Set(crate_id),
                author_fk: Set(author_fk),
            };
            catc.insert(&self.db_con).await?;
        }

        Ok(())
    }

    async fn compute_etag(&self, crate_name: &str, crate_id: i64) -> DbResult<String> {
        let crate_indices = crate_index::Entity::find()
            .filter(crate_index::Column::CrateFk.eq(crate_id))
            .all(&self.db_con)
            .await?;

        let index_metadata = Self::crate_index_model_to_index_metadata(crate_name, crate_indices)?;
        let data = Self::index_metadata_to_bytes(&index_metadata)?;

        Ok(sha256::digest(data))
    }

    fn index_metadata_to_bytes(index_metadata: &[IndexMetadata]) -> DbResult<Vec<u8>> {
        IndexMetadata::serialize_indices(index_metadata)
            .map(|idx| idx.into_bytes())
            .map_err(|e| DbError::FailedToConvertToJson(format!("{e}")))
    }

    fn crate_index_model_to_index_metadata(
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
                links: ci.links,
                v: Some(ci.v as u32),
                features2: None,
            };
            index_metadata.push(cm);
        }
        Ok(index_metadata)
    }

    fn cratesio_index_model_to_index_metadata(
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
                vers: ci.vers.to_string(),
                deps,
                cksum: ci.cksum.to_string(),
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

    async fn update_etag(&self, crate_name: &str, crate_id: i64) -> DbResult<()> {
        let etag = self.compute_etag(crate_name, crate_id).await?;
        let krate = krate::Entity::find()
            .filter(krate::Column::Id.eq(crate_id))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::CrateNotFound(crate_name.to_string()))?;
        let mut krate: krate::ActiveModel = krate.into();
        krate.e_tag = Set(etag);
        krate.update(&self.db_con).await?;
        Ok(())
    }
}

#[async_trait]
impl DbProvider for Database {
    async fn get_total_unique_cached_crates(&self) -> DbResult<u64> {
        let stmt = Query::select()
            .expr_as(
                Expr::col((CratesIoIden::Table, CratesIoIden::Id)).count(),
                Alias::new("count"),
            )
            .from(CratesIoIden::Table)
            .to_owned();

        #[derive(Debug, PartialEq, FromQueryResult)]
        struct SelectResult {
            count: Option<i64>,
        }

        let builder = self.db_con.get_database_backend();
        let result = SelectResult::find_by_statement(builder.build(&stmt))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::FailedToCountCrates)?
            .count
            .ok_or(DbError::FailedToCountCrates)?;

        Ok(result as u64)
    }

    async fn get_total_cached_crate_versions(&self) -> DbResult<u64> {
        let stmt = Query::select()
            .expr_as(
                Expr::col((CratesIoMetaIden::Table, CratesIoMetaIden::Id)).count(),
                Alias::new("count"),
            )
            .from(CratesIoMetaIden::Table)
            .to_owned();

        #[derive(Debug, PartialEq, FromQueryResult)]
        struct SelectResult {
            count: Option<i64>,
        }

        let builder = self.db_con.get_database_backend();
        let result = SelectResult::find_by_statement(builder.build(&stmt))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::FailedToCountCrateVersions)?
            .count
            .ok_or(DbError::FailedToCountCrateVersions)?;

        Ok(result as u64)
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
        let krate: krate::Model = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?;
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
            let name = OriginalName::from_unchecked_str(krate.original_name);
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
        let user_fk = user::Entity::find()
            .filter(user::Column::Name.eq(user))
            .one(&self.db_con)
            .await?
            .map(|model| model.id)
            .ok_or_else(|| DbError::UserNotFound(user.to_string()))?;

        let crate_fk: i64 = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
            .map(|model| model.id)
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?;

        let u = crate_user::ActiveModel {
            user_fk: Set(user_fk),
            crate_fk: Set(crate_fk),
            ..Default::default()
        };

        CrateUser::insert(u).exec(&self.db_con).await?;
        Ok(())
    }

    async fn add_owner(&self, crate_name: &NormalizedName, owner: &str) -> DbResult<()> {
        let user_fk = user::Entity::find()
            .filter(user::Column::Name.eq(owner))
            .one(&self.db_con)
            .await?
            .map(|model| model.id)
            .ok_or_else(|| DbError::UserNotFound(owner.to_string()))?;

        let crate_fk: i64 = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
            .map(|model| model.id)
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?;

        let o = owner::ActiveModel {
            user_fk: Set(user_fk),
            crate_fk: Set(crate_fk),
            ..Default::default()
        };

        Owner::insert(o).exec(&self.db_con).await?;
        Ok(())
    }

    async fn is_download_restricted(&self, crate_name: &NormalizedName) -> DbResult<bool> {
        let restricted_download = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
            .map(|model| model.restricted_download);

        match restricted_download {
            Some(restricted) => Ok(restricted),
            None => Ok(false),
        }
    }

    async fn change_download_restricted(
        &self,
        crate_name: &NormalizedName,
        restricted: bool,
    ) -> DbResult<()> {
        let mut krate: krate::ActiveModel = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?
            .into();

        krate.restricted_download = Set(restricted);
        krate.update(&self.db_con).await?;
        Ok(())
    }

    async fn is_crate_user(&self, crate_name: &NormalizedName, user: &str) -> DbResult<bool> {
        let user = crate_user::Entity::find()
            .join(JoinType::InnerJoin, crate_user::Relation::Krate.def())
            .join(JoinType::InnerJoin, crate_user::Relation::User.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name.to_string()))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await?;

        Ok(user.is_some())
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

        Ok(u.into_iter()
            .map(|u| User {
                id: u.id as i32,
                name: u.name,
                pwd: u.pwd,
                salt: u.salt,
                is_admin: u.is_admin,
            })
            .collect())
    }

    async fn get_crate_users(&self, crate_name: &NormalizedName) -> DbResult<Vec<User>> {
        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::CrateUser.def())
            .join(JoinType::InnerJoin, crate_user::Relation::Krate.def())
            .filter(Expr::col((CrateIden::Table, krate::Column::Name)).eq(crate_name.to_string()))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter()
            .map(|u| User {
                id: u.id as i32,
                name: u.name,
                pwd: u.pwd,
                salt: u.salt,
                is_admin: u.is_admin,
            })
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
        let u = user::Entity::find()
            .filter(user::Column::Name.eq(user_name))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::UserNotFound(user_name.to_owned()))?;

        u.delete(&self.db_con).await?;
        Ok(())
    }

    async fn change_pwd(&self, user_name: &str, new_pwd: &str) -> DbResult<()> {
        let salt = generate_salt();
        let hashed = hash_pwd(new_pwd, &salt);

        let mut u: user::ActiveModel = user::Entity::find()
            .filter(user::Column::Name.eq(user_name))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::UserNotFound(user_name.to_owned()))?
            .into();

        u.pwd = Set(hashed.to_owned());
        u.salt = Set(salt);

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
        let krate = krate::Entity::find_by_id(crate_id)
            .one(&self.db_con)
            .await?;

        let k = krate.ok_or(DbError::FailedToGetMaxVersionById(crate_id))?;
        let v = Version::try_from(&k.max_version)
            .map_err(|_| DbError::FailedToGetMaxVersionById(crate_id))?;
        Ok(v)
    }

    async fn get_max_version_from_name(&self, crate_name: &NormalizedName) -> DbResult<Version> {
        let krate = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?;

        let k =
            krate.ok_or_else(|| DbError::FailedToGetMaxVersionByName(crate_name.to_string()))?;
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
        let user = user::Entity::find()
            .filter(user::Column::Name.eq(user))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::UserNotFound(user.to_string()))?;

        let at = auth_token::ActiveModel {
            name: Set(name.to_owned()),
            token: Set(token.to_owned()),
            user_fk: Set(user.id),
            ..Default::default()
        };

        at.insert(&self.db_con).await?;

        Ok(())
    }

    async fn get_user_from_token(&self, token: &str) -> DbResult<User> {
        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::AuthToken.def())
            .filter(Expr::col((AuthTokenIden::Table, AuthTokenIden::Token)).eq(token))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::TokenNotFound)?;

        Ok(User {
            id: u.id as i32,
            name: u.name,
            pwd: u.pwd,
            salt: u.salt,
            is_admin: u.is_admin,
        })
    }

    async fn get_user(&self, name: &str) -> DbResult<User> {
        let u = user::Entity::find()
            .filter(user::Column::Name.eq(name))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::UserNotFound(name.to_owned()))?;

        Ok(User {
            id: u.id as i32,
            name: u.name,
            pwd: u.pwd,
            salt: u.salt,
            is_admin: u.is_admin,
        })
    }

    async fn get_auth_tokens(&self, user_name: &str) -> DbResult<Vec<AuthToken>> {
        let at: Vec<auth_token::Model> = auth_token::Entity::find()
            .join(JoinType::InnerJoin, auth_token::Relation::User.def())
            .filter(user::Column::Name.eq(user_name))
            .all(&self.db_con)
            .await?;

        Ok(at
            .into_iter()
            .map(|x| AuthToken::new(x.id as i32, x.name, x.token))
            .collect())
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

    async fn delete_crate_user(&self, crate_name: &str, user: &str) -> DbResult<()> {
        let user = crate_user::Entity::find()
            .join(JoinType::InnerJoin, crate_user::Relation::Krate.def())
            .join(JoinType::InnerJoin, crate_user::Relation::User.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::UserNotFound(user.to_string()))?;

        user.delete(&self.db_con).await?;

        Ok(())
    }

    async fn add_user(&self, name: &str, pwd: &str, salt: &str, is_admin: bool) -> DbResult<()> {
        let hashed_pwd = hash_pwd(pwd, salt);

        let u = user::ActiveModel {
            name: Set(name.to_owned()),
            pwd: Set(hashed_pwd),
            salt: Set(salt.to_owned()),
            is_admin: Set(is_admin),
            ..Default::default()
        };

        u.insert(&self.db_con).await?;
        Ok(())
    }

    async fn get_users(&self) -> DbResult<Vec<User>> {
        let users = user::Entity::find()
            .order_by_asc(user::Column::Name)
            .all(&self.db_con)
            .await?;

        Ok(users
            .into_iter()
            .map(|u| User {
                id: u.id as i32,
                name: u.name,
                pwd: u.pwd,
                salt: u.salt,
                is_admin: u.is_admin,
            })
            .collect())
    }

    async fn get_total_unique_crates(&self) -> DbResult<u32> {
        let stmt = Query::select()
            .expr_as(
                Expr::col((CrateIden::Table, CrateIden::Id)).count(),
                Alias::new("count"),
            )
            .from(CrateIden::Table)
            .to_owned();

        #[derive(Debug, PartialEq, FromQueryResult)]
        struct SelectResult {
            count: Option<i64>,
        }

        let builder = self.db_con.get_database_backend();
        let result = SelectResult::find_by_statement(builder.build(&stmt))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::FailedToCountCrates)?
            .count
            .ok_or(DbError::FailedToCountCrates)?;

        Ok(result as u32)
    }

    async fn get_total_crate_versions(&self) -> DbResult<u32> {
        let stmt = Query::select()
            .expr_as(
                Expr::col((CrateMetaIden::Table, CrateMetaIden::Id)).count(),
                Alias::new("count"),
            )
            .from(CrateMetaIden::Table)
            .to_owned();

        #[derive(Debug, PartialEq, FromQueryResult)]
        struct SelectResult {
            count: Option<i64>,
        }

        let builder = self.db_con.get_database_backend();
        let result = SelectResult::find_by_statement(builder.build(&stmt))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::FailedToCountCrateVersions)?
            .count
            .ok_or(DbError::FailedToCountCrateVersions)?;

        Ok(result as u32)
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
        let stmt = Query::select()
            .columns(vec![CrateIden::OriginalName, CrateIden::TotalDownloads])
            .from(CrateIden::Table)
            .order_by(CrateIden::TotalDownloads, Order::Desc)
            .limit(top as u64)
            .to_owned();

        #[derive(Debug, PartialEq, FromQueryResult)]
        struct SelectResult {
            original_name: String,
            total_downloads: i64,
        }

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

        let krates = krates
            .iter()
            .map(|c| CrateSummary {
                name: c.name.clone(),
                max_version: c.max_version.clone(),
                last_updated: c.last_updated.clone(),
                total_downloads: c.total_downloads,
            })
            .collect();

        Ok(krates)
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
        // Delete the entry from the "crate_meta" table
        let crate_meta_version = crate_meta::Entity::find()
            .join(JoinType::InnerJoin, crate_meta::Relation::Krate.def())
            .filter(krate::Column::Name.eq(krate.to_string()))
            .filter(crate_meta::Column::Version.eq(version.to_string()))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateMetaNotFound(krate.to_string(), version.to_string()))?;
        let crate_id = crate_meta_version.crate_fk;
        let current_max_version = self.get_max_version_from_id(crate_id).await?;
        crate_meta_version.delete(&self.db_con).await?;

        // Delete the crate index entry from "crate_index" table
        let crate_index_version = crate_index::Entity::find()
            .join(JoinType::InnerJoin, crate_index::Relation::Krate.def())
            .filter(krate::Column::Name.eq(krate.to_string()))
            .filter(crate_index::Column::Vers.eq(version.to_string()))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateIndexNotFound(krate.to_string(), version.to_string()))?;
        crate_index_version.delete(&self.db_con).await?;

        // If it was the last entry in the "crate_meta" table, delete the entry
        // in the "crate" table as well
        let crate_meta_rows = crate_meta::Entity::find()
            .join(JoinType::InnerJoin, crate_meta::Relation::Krate.def())
            .filter(krate::Column::Name.eq(krate.to_string()))
            .all(&self.db_con)
            .await?;

        if crate_meta_rows.is_empty() {
            krate::Entity::delete_many()
                .filter(krate::Column::Name.eq(krate.to_string()))
                .exec(&self.db_con)
                .await?;
        } else {
            let c = krate::Entity::find_by_id(crate_id)
                .one(&self.db_con)
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
            let etag = self.compute_etag(krate, crate_id).await?;
            c.e_tag = Set(etag);
            c.update(&self.db_con).await?;
        }

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
            .and_where(
                Expr::col((CrateIden::Table, CrateIden::Name)).like(format!("%{}%", contains)),
            )
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
                                .like(format!("%{}%", contains)),
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
        let krate = krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
            .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?;

        let owners: Vec<String> = krate
            .find_related(owner::Entity)
            .find_also_related(user::Entity)
            .all(&self.db_con)
            .await?
            .into_iter()
            .map(|(_, u)| u)
            .filter(|u| u.is_some())
            .map(|u| u.unwrap().name)
            .collect();
        let categories: Vec<String> = krate
            .find_related(crate_category_to_crate::Entity)
            .find_also_related(crate_category::Entity)
            .all(&self.db_con)
            .await?
            .into_iter()
            .map(|(_, c)| c)
            .filter(|c| c.is_some())
            .map(|c| c.unwrap().category)
            .collect();
        let keywords: Vec<String> = krate
            .find_related(crate_keyword_to_crate::Entity)
            .find_also_related(crate_keyword::Entity)
            .all(&self.db_con)
            .await?
            .into_iter()
            .map(|(_, k)| k)
            .filter(|k| k.is_some())
            .map(|k| k.unwrap().keyword)
            .collect();
        let authors: Vec<String> = krate
            .find_related(crate_author_to_crate::Entity)
            .find_also_related(crate_author::Entity)
            .all(&self.db_con)
            .await?
            .into_iter()
            .map(|(_, a)| a)
            .filter(|a| a.is_some())
            .map(|a| a.unwrap().author)
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
                            self.get_desc_for_crate_dep(&dep.name, &dep.registry)
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
            })
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

        let crate_id = match krate::Entity::find()
            .filter(krate::Column::Name.eq(pub_metadata.name.clone()))
            .one(&self.db_con)
            .await?
        {
            Some(krate) => {
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
                krate.e_tag = Set("".to_string()); // Set to empty string, as it can be computed, when the crate index is inserted
                krate.update(&self.db_con).await?;
                krate_id
            }
            None => {
                let krate = krate::ActiveModel {
                    id: Default::default(),
                    name: Set(normalized_name.to_string()),
                    original_name: Set(pub_metadata.name.clone()),
                    max_version: Set(pub_metadata.vers.clone()),
                    last_updated: Set(created.clone()),
                    total_downloads: Set(0),
                    homepage: Set(pub_metadata.homepage.clone()),
                    description: Set(pub_metadata.description.clone()),
                    repository: Set(pub_metadata.repository.clone()),
                    e_tag: Set("".to_string()), // Set to empty string, as it can be computed, when the crate index is inserted
                    restricted_download: Set(false),
                };
                let krate = krate.insert(&self.db_con).await?;
                krate.id
            }
        };

        self.add_owner_if_not_exists(owner, crate_id).await?;
        self.add_crate_metadata(pub_metadata, &created, crate_id)
            .await?;
        self.add_crate_index(pub_metadata, cksum, crate_id).await?;
        self.update_etag(&pub_metadata.name, crate_id).await?;
        self.update_crate_categories(pub_metadata, crate_id).await?;
        self.update_crate_keywords(pub_metadata, crate_id).await?;
        self.update_crate_authors(pub_metadata, crate_id).await?;
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
        let cm = crate_meta::ActiveModel {
            id: Default::default(),
            version: Set(pub_metadata.vers.to_string()),
            created: Set(created.to_string()),
            downloads: Set(0),
            crate_fk: Set(crate_id),
            readme: Set(pub_metadata.readme.clone()),
            license: Set(pub_metadata.license.clone()),
            license_file: Set(pub_metadata.license_file.clone()),
            documentation: Set(pub_metadata.documentation.clone()),
        };

        cm.insert(&self.db_con).await?;

        Ok(())
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
        let index_metadata = Self::crate_index_model_to_index_metadata(crate_name, crate_indices)?;
        let data = Self::index_metadata_to_bytes(&index_metadata)?;

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
        let krate = match cratesio_crate::Entity::find()
            .filter(cratesio_crate::Column::Name.eq(crate_name.to_string()))
            .one(&self.db_con)
            .await?
        {
            Some(krate) => krate,
            None => return Ok(PrefetchState::NotFound),
        };

        let needs_update = match (etag, last_modified) {
            (Some(etag), Some(last_modified)) => {
                krate.e_tag != etag || krate.last_modified != last_modified
            }
            (Some(etag), None) => krate.e_tag != etag,
            (None, Some(last_modified)) => krate.last_modified != last_modified,
            (_, _) => true,
        };

        if !needs_update {
            Ok(PrefetchState::UpToDate)
        } else {
            let crate_indices = krate
                .find_related(cratesio_index::Entity)
                .all(&self.db_con)
                .await?;
            let index_metadata =
                Self::cratesio_index_model_to_index_metadata(crate_name, crate_indices)?;
            let data = Self::index_metadata_to_bytes(&index_metadata)?;

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

        let krate = match cratesio_crate::Entity::find()
            .filter(cratesio_crate::Column::Name.eq(normalized_name.to_string()))
            .one(&self.db_con)
            .await?
        {
            Some(krate) => {
                let mut krate: cratesio_crate::ActiveModel = krate.into();
                krate.e_tag = Set(etag.to_string());
                krate.last_modified = Set(last_modified.to_string());
                krate.max_version = Set(max_version.to_string());
                krate.update(&self.db_con).await?
            }
            None => {
                let krate = cratesio_crate::ActiveModel {
                    id: Default::default(),
                    name: Set(normalized_name.to_string()),
                    original_name: Set(crate_name.to_string()),
                    description: Set(description),
                    e_tag: Set(etag.to_string()),
                    last_modified: Set(last_modified.to_string()),
                    total_downloads: Set(0),
                    max_version: Set(max_version.to_string()),
                };
                krate.insert(&self.db_con).await?
            }
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
                    id: Default::default(),
                    name: Set(index.name.clone()),
                    vers: Set(index.vers.clone()),
                    deps: Set(deps),
                    cksum: Set(index.cksum.clone()),
                    features: Set(Some(features)),
                    features2: Set(Some(features2)),
                    yanked: Set(index.yanked),
                    links: Set(index.links.clone()),
                    v: Set(index.v.unwrap_or(1) as i32),
                    crates_io_fk: Set(krate.id),
                };

                new_index.insert(&self.db_con).await?;

                // Add the meta data for the crate version.
                let meta = cratesio_meta::ActiveModel {
                    id: Default::default(),
                    version: Set(index.vers.clone()),
                    downloads: Set(0),
                    crates_io_fk: Set(krate.id),
                    documentation: Set(Some(format!(
                        "https://docs.rs/{}/{}",
                        normalized_name, index.vers,
                    ))),
                };

                meta.insert(&self.db_con).await?;
            }
        }

        Ok(Prefetch {
            data: Self::index_metadata_to_bytes(indices)?,
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
                    name: OriginalName::from_unchecked_str(krate.original_name),
                    etag: Some(krate.e_tag),
                    last_modified: Some(krate.last_modified),
                })
            })
            .collect();
        Ok(msgs)
    }

    async fn unyank_crate(&self, crate_name: &NormalizedName, version: &Version) -> DbResult<()> {
        let ci = crate_index::Entity::find()
            .filter(crate_index::Column::Name.eq(crate_name.to_string()))
            .filter(crate_index::Column::Vers.eq(version.to_string()))
            .one(&self.db_con)
            .await?;

        let mut ci: crate_index::ActiveModel = ci
            .ok_or(DbError::CrateIndexNotFound(
                crate_name.to_string(),
                version.to_string(),
            ))?
            .into();

        ci.yanked = Set(false);
        ci.save(&self.db_con).await?;

        Ok(())
    }

    async fn yank_crate(&self, crate_name: &NormalizedName, version: &Version) -> DbResult<()> {
        let ci = crate_index::Entity::find()
            .filter(crate_index::Column::Name.eq(crate_name.to_string()))
            .filter(crate_index::Column::Vers.eq(version.to_string()))
            .one(&self.db_con)
            .await?;

        let mut ci: crate_index::ActiveModel = ci
            .ok_or(DbError::CrateIndexNotFound(
                crate_name.to_string(),
                version.to_string(),
            ))?
            .into();

        ci.yanked = Set(true);
        ci.save(&self.db_con).await?;

        Ok(())
    }
}
