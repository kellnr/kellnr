use std::path::Path;

use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_entity::doc_queue;
use kellnr_entity::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use tracing::warn;

use super::Database;
use crate::DocQueueEntry;
use crate::error::DbError;
use crate::provider::DbResult;

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
}
