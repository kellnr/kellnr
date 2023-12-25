use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use std::path::PathBuf;

#[derive(Eq, PartialEq, Debug)]
pub struct DocQueueEntry {
    pub id: i64,
    pub krate: NormalizedName,
    pub version: String,
    pub path: PathBuf,
}

impl From<entity::doc_queue::Model> for DocQueueEntry {
    fn from(dqm: entity::doc_queue::Model) -> Self {
        Self {
            id: dqm.id,
            krate: OriginalName::from_unchecked_str(dqm.krate).to_normalized(),
            version: dqm.version,
            path: PathBuf::from(dqm.path),
        }
    }
}
