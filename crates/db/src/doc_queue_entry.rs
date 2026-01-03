use std::path::PathBuf;

use kellnr_common::normalized_name::NormalizedName;

#[derive(Eq, PartialEq, Debug)]
pub struct DocQueueEntry {
    pub id: i64,
    pub normalized_name: NormalizedName,
    pub version: String,
    pub path: PathBuf,
}

impl From<kellnr_entity::doc_queue::Model> for DocQueueEntry {
    fn from(dqm: kellnr_entity::doc_queue::Model) -> Self {
        Self {
            id: dqm.id,
            normalized_name: NormalizedName::from_unchecked(dqm.krate),
            version: dqm.version,
            path: PathBuf::from(dqm.path),
        }
    }
}
