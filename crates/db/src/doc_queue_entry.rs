use common::normalized_name::NormalizedName;
use std::path::PathBuf;

#[derive(Eq, PartialEq, Debug)]
pub struct DocQueueEntry {
    pub id: i64,
    pub normalized_name: NormalizedName,
    pub version: String,
    pub path: PathBuf,
}

impl From<entity::doc_queue::Model> for DocQueueEntry {
    fn from(dqm: entity::doc_queue::Model) -> Self {
        Self {
            id: dqm.id,
            normalized_name: NormalizedName::from_unchecked(dqm.krate),
            version: dqm.version,
            path: PathBuf::from(dqm.path),
        }
    }
}
