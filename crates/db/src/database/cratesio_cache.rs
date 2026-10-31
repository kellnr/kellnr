use std::collections::HashMap;

use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::cratesio_prefetch_msg::{CratesioPrefetchMsg, UpdateData};
use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::original_name::OriginalName;
use kellnr_common::prefetch::{Prefetch, is_not_modified};
use kellnr_common::version::Version;
use kellnr_entity::{cratesio_crate, cratesio_index, cratesio_meta};
use kellnr_migration::http_date_to_db_date;
use sea_orm::query::{QueryOrder, QuerySelect};
use sea_orm::sea_query::{Condition, Expr, JoinType};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, ExprTrait, FromQueryResult,
    ModelTrait, QueryFilter, RelationTrait, Set,
};

use super::{Database, operations, valid_version};
use crate::error::DbError;
use crate::provider::{CratesioVersionMeta, DbResult, PrefetchState};

impl Database {
    pub(super) async fn is_cratesio_cache_up_to_date(
        &self,
        crate_name: &OriginalName,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> DbResult<PrefetchState> {
        // Like the index of crates.io, the index file is only found under the
        // separators the crate was published with.
        let Some(krate) = cratesio_crate::Entity::find()
            .filter(cratesio_crate::Column::Name.eq(crate_name.to_canonical()))
            .one(&self.db_con)
            .await?
            .filter(|krate| krate.original_name.to_lowercase() == crate_name.to_lowercase())
        else {
            return Ok(PrefetchState::NotFound);
        };

        let needs_update = !is_not_modified(
            etag.as_deref(),
            last_modified.as_deref(),
            &krate.e_tag,
            &krate.last_modified,
        );

        if !needs_update {
            Ok(PrefetchState::UpToDate {
                etag: krate.e_tag,
                last_modified: krate.last_modified,
            })
        } else {
            let crate_indices = krate
                .find_related(cratesio_index::Entity)
                .order_by_asc(cratesio_index::Column::Id)
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

    pub(super) async fn add_cratesio_prefetch_data(
        &self,
        crate_name: &OriginalName,
        etag: &str,
        last_modified: &str,
        description: Option<String>,
        indices: &[IndexMetadata],
        licenses: &HashMap<String, String>,
    ) -> DbResult<Prefetch> {
        let canonical_name = crate_name.to_canonical();

        let max_version = indices
            .iter()
            .filter_map(|i| valid_version(&i.vers, crate_name))
            .max()
            .ok_or_else(|| DbError::FailedToGetMaxVersionByName(crate_name.to_string()))?;

        // Use a transaction to ensure the etag update and all index inserts are
        // atomic. Without this, a partial failure would leave the crate record
        // with the latest etag but incomplete index data, causing stale cache
        // entries that can never self-heal (the background updater would get 304
        // from crates.io because the etag matches).
        self.in_transaction(async |txn| {
            let krate = cratesio_crate::Entity::find()
                .filter(cratesio_crate::Column::Name.eq(&canonical_name))
                .one(txn)
                .await?;

            // The Last-Modified header is kept as is for conditional requests to
            // crates.io, the date is stored separately to sort by it.
            let last_updated = http_date_to_db_date(last_modified);

            let krate = if let Some(krate) = krate {
                let mut krate: cratesio_crate::ActiveModel = krate.into();
                krate.e_tag = Set(etag.to_string());
                krate.last_modified = Set(last_modified.to_string());
                if let Some(last_updated) = last_updated {
                    krate.last_updated = Set(last_updated);
                }
                if let Some(description) = description {
                    krate.description = Set(Some(description));
                }
                krate.max_version = Set(max_version.to_string());
                krate.update(txn).await?
            } else {
                let krate = cratesio_crate::ActiveModel {
                    id: ActiveValue::default(),
                    name: Set(canonical_name.to_string()),
                    original_name: Set(crate_name.to_string()),
                    description: Set(description),
                    e_tag: Set(etag.to_string()),
                    last_modified: Set(last_modified.to_string()),
                    last_updated: Set(last_updated.unwrap_or_default()),
                    total_downloads: Set(0),
                    max_version: Set(max_version.to_string()),
                };
                krate.insert(txn).await?
            };

            let current_indices = cratesio_index::Entity::find()
                .filter(cratesio_index::Column::CratesIoFk.eq(krate.id))
                .all(txn)
                .await?;

            for index in indices {
                // Refresh the mutable parts of an already cached version. "yanked"
                // can flip upstream at any time. "rust_version" is refreshed too
                // because caches written before the column existed hold NULL for
                // every row, and only a refresh can heal them.
                if let Some(current_index) = current_indices.iter().find(|ci| index.vers == ci.vers)
                {
                    let mut ci: cratesio_index::ActiveModel = current_index.to_owned().into();
                    let mut changed = false;

                    if index.yanked != current_index.yanked {
                        ci.yanked = Set(index.yanked);
                        changed = true;
                    }

                    if index.rust_version != current_index.rust_version {
                        ci.rust_version = Set(index.rust_version.clone());
                        changed = true;
                    }

                    if changed {
                        ci.update(txn).await?;
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
                        rust_version: Set(index.rust_version.clone()),
                    };

                    new_index.insert(txn).await?;

                    // Add the meta data for the crate version.
                    let meta = cratesio_meta::ActiveModel {
                        id: ActiveValue::default(),
                        version: Set(index.vers.clone()),
                        downloads: Set(0),
                        crates_io_fk: Set(krate.id),
                        documentation: Set(Some(format!(
                            "https://docs.rs/{}/{}",
                            crate_name.to_lowercase(),
                            index.vers,
                        ))),
                        license: Set(licenses.get(&index.vers).cloned()),
                    };

                    meta.insert(txn).await?;
                }
            }

            // The license of a published version cannot change upstream, so only
            // versions cached before it was fetched successfully get it here.
            let unlicensed = cratesio_meta::Entity::find()
                .filter(cratesio_meta::Column::CratesIoFk.eq(krate.id))
                .filter(cratesio_meta::Column::License.is_null())
                .all(txn)
                .await?;
            for meta in unlicensed {
                if let Some(license) = licenses.get(&meta.version) {
                    let mut meta: cratesio_meta::ActiveModel = meta.into();
                    meta.license = Set(Some(license.clone()));
                    meta.update(txn).await?;
                }
            }

            Ok(())
        })
        .await?;

        Ok(Prefetch {
            data: operations::index_metadata_to_bytes(indices)?,
            etag: etag.to_string(),
            last_modified: last_modified.to_string(),
        })
    }

    pub(super) async fn get_cratesio_index_update_list(
        &self,
    ) -> DbResult<Vec<CratesioPrefetchMsg>> {
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

    pub(super) async fn get_cratesio_version_meta(
        &self,
        crate_name: &CanonicalName,
        version: &Version,
    ) -> DbResult<Option<CratesioVersionMeta>> {
        #[derive(FromQueryResult)]
        struct Meta {
            cksum: String,
            license: Option<String>,
        }

        // One query, as it runs for every download from the crates.io proxy
        let version = version.to_string();
        let meta_version = version.clone();
        let meta_of_version =
            cratesio_crate::Relation::CratesioMeta
                .def()
                .on_condition(move |_crate, meta| {
                    Condition::all().add(
                        Expr::col((meta, cratesio_meta::Column::Version)).eq(meta_version.clone()),
                    )
                });
        let meta = cratesio_index::Entity::find()
            .select_only()
            .column(cratesio_index::Column::Cksum)
            .column(cratesio_meta::Column::License)
            .inner_join(cratesio_crate::Entity)
            .join(JoinType::LeftJoin, meta_of_version)
            .filter(cratesio_crate::Column::Name.eq(crate_name))
            .filter(cratesio_index::Column::Vers.eq(version))
            .into_model::<Meta>()
            .one(&self.db_con)
            .await?;

        Ok(meta.map(|m| CratesioVersionMeta {
            cksum: m.cksum,
            license: m.license,
        }))
    }
}
