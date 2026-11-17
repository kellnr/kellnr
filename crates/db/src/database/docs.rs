use std::path::Path;

use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_entity::prelude::*;
use kellnr_entity::{crate_meta, doc_build, doc_queue, krate};
use sea_orm::query::{QueryOrder, QuerySelect};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, RelationTrait, Set};
use tracing::warn;

use super::{DB_DATE_FORMAT, Database, parse_db_date, parse_db_version};
use crate::error::DbError;
use crate::provider::DbResult;
use crate::{DocBuild, DocBuildStatus, DocQueueEntry, RecordedDocBuild};

impl Database {
    pub(super) async fn add_doc_queue(
        &self,
        krate: &OriginalName,
        version: &Version,
        path: &Path,
    ) -> DbResult<()> {
        let s = doc_queue::ActiveModel {
            // Lowercase, as the queue showed it before crate names were canonical
            krate: Set(krate.to_lowercase()),
            version: Set(version.to_string()),
            path: Set(path
                .to_str()
                .ok_or_else(|| DbError::NonUtf8Path(path.display().to_string()))?
                .to_string()),
            ..Default::default()
        };

        s.insert(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn delete_doc_queue(&self, id: i64) -> DbResult<()> {
        DocQueue::delete_by_id(id).exec(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn get_doc_queue(&self) -> DbResult<Vec<DocQueueEntry>> {
        let entities = DocQueue::find().all(&self.db_con).await?;

        let mut entries = Vec::with_capacity(entities.len());
        for entity in entities {
            let (id, krate, version) = (entity.id, entity.krate.clone(), entity.version.clone());
            if let Ok(entry) = DocQueueEntry::try_from(entity) {
                entries.push(entry);
            } else {
                // The docs of an invalid version can never be built.
                warn!("Removing docs build of {krate} {version}, the version is invalid");
                DocQueue::delete_by_id(id).exec(&self.db_con).await?;
            }
        }
        Ok(entries)
    }

    pub(super) async fn add_doc_build(&self, build: &DocBuild, keep: u64) -> DbResult<()> {
        self.in_transaction(async |txn| {
            let crate_meta_id: i64 = crate_meta::Entity::find()
                .select_only()
                .column(crate_meta::Column::Id)
                .inner_join(krate::Entity)
                .filter(krate::Column::Name.eq(build.name.to_canonical()))
                .filter(crate_meta::Column::Version.eq(build.version.to_string()))
                .into_tuple()
                .one(txn)
                .await?
                .ok_or_else(|| {
                    DbError::CrateMetaNotFound(build.name.to_string(), build.version.to_string())
                })?;

            doc_build::ActiveModel {
                crate_meta_fk: Set(crate_meta_id),
                status: Set(build.status.as_str().to_string()),
                error: Set(build.error.clone()),
                started: Set(build.started.format(DB_DATE_FORMAT).to_string()),
                finished: Set(build.finished.format(DB_DATE_FORMAT).to_string()),
                ..Default::default()
            }
            .insert(txn)
            .await?;

            // Everything older than the newest `keep` builds is removed.
            let oldest_removed: Option<i64> = doc_build::Entity::find()
                .select_only()
                .column(doc_build::Column::Id)
                .order_by_desc(doc_build::Column::Id)
                .offset(keep)
                .into_tuple()
                .one(txn)
                .await?;
            if let Some(id) = oldest_removed {
                doc_build::Entity::delete_many()
                    .filter(doc_build::Column::Id.lte(id))
                    .exec(txn)
                    .await?;
            }
            Ok(())
        })
        .await
    }

    pub(super) async fn get_doc_builds(&self, limit: u64) -> DbResult<Vec<RecordedDocBuild>> {
        type Row = (i64, String, String, String, Option<String>, String, String);
        let rows: Vec<Row> = doc_build::Entity::find()
            .select_only()
            .column(doc_build::Column::Id)
            .column(krate::Column::OriginalName)
            .column(crate_meta::Column::Version)
            .column(doc_build::Column::Status)
            .column(doc_build::Column::Error)
            .column(doc_build::Column::Started)
            .column(doc_build::Column::Finished)
            .inner_join(crate_meta::Entity)
            .join(
                sea_orm::JoinType::InnerJoin,
                crate_meta::Relation::Krate.def(),
            )
            .order_by_desc(doc_build::Column::Id)
            .limit(limit)
            .into_tuple()
            .all(&self.db_con)
            .await?;

        rows.into_iter()
            .map(|(id, name, version, status, error, started, finished)| {
                let build = DocBuild {
                    name: OriginalName::try_from(name.as_str())
                        .map_err(|_| DbError::InvalidCrateName(name.clone()))?,
                    version: parse_db_version(&version)?,
                    status: DocBuildStatus::from_db(&status),
                    error,
                    started: parse_db_date(&started)?,
                    finished: parse_db_date(&finished)?,
                };
                Ok(RecordedDocBuild { id, build })
            })
            .collect()
    }
}
