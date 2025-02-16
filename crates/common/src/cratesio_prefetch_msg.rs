use crate::{normalized_name::NormalizedName, original_name::OriginalName, version::Version};

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
    pub name: NormalizedName,
    pub version: Version,
}

pub enum CratesioPrefetchMsg {
    Insert(InsertData),
    Update(UpdateData),
    IncDownloadCnt(DownloadData),
}
