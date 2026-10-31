use std::path::PathBuf;

use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::original_name::NameError;
use kellnr_common::version::{Version, VersionError};

#[derive(Eq, PartialEq, Debug)]
pub struct DocQueueEntry {
    pub id: i64,
    /// The crate name in lowercase, with the separators it was published with
    pub name: String,
    pub version: Version,
    pub path: PathBuf,
}

impl DocQueueEntry {
    pub fn canonical_name(&self) -> Result<CanonicalName, NameError> {
        CanonicalName::try_from(self.name.as_str())
    }
}

impl TryFrom<kellnr_entity::doc_queue::Model> for DocQueueEntry {
    type Error = VersionError;

    fn try_from(dqm: kellnr_entity::doc_queue::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: dqm.id,
            name: dqm.krate,
            version: Version::try_from(dqm.version.as_str())?,
            path: PathBuf::from(dqm.path),
        })
    }
}
