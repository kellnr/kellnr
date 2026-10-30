use std::path::PathBuf;

use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::original_name::NameError;

#[derive(Eq, PartialEq, Debug)]
pub struct DocQueueEntry {
    pub id: i64,
    /// The crate name in lowercase, with the separators it was published with
    pub name: String,
    pub version: String,
    pub path: PathBuf,
}

impl DocQueueEntry {
    pub fn canonical_name(&self) -> Result<CanonicalName, NameError> {
        CanonicalName::try_from(self.name.as_str())
    }
}

impl From<kellnr_entity::doc_queue::Model> for DocQueueEntry {
    fn from(dqm: kellnr_entity::doc_queue::Model) -> Self {
        Self {
            id: dqm.id,
            name: dqm.krate,
            version: dqm.version,
            path: PathBuf::from(dqm.path),
        }
    }
}
