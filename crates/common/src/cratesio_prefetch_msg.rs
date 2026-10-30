use crate::canonical_name::CanonicalName;
use crate::original_name::OriginalName;
use crate::version::Version;

pub struct InsertData {
    pub name: OriginalName,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub data: String,
}

pub struct UpdateData {
    pub name: OriginalName,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

pub struct DownloadData {
    pub name: CanonicalName,
    pub version: Version,
}

pub enum CratesioPrefetchMsg {
    Insert(InsertData),
    Update(UpdateData),
    IncDownloadCnt(DownloadData),
}
