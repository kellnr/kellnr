use crate::original_name::OriginalName;

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

pub enum CratesioPrefetchMsg {
    Insert(InsertData),
    Update(UpdateData),
}
