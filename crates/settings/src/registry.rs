use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Registry {
    pub data_dir: String,
    pub session_age_seconds: u64,
    pub cache_size: u64,
    pub max_crate_size: u64,
    pub max_db_connections: u32,
    pub auth_required: LegacyAuthRequiredWrapper,
    pub required_crate_fields: Vec<String>,
    pub new_crates_restricted: bool,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            data_dir: "/tmp/kellnr".to_string(),
            session_age_seconds: 60 * 60 * 8,
            cache_size: 1000,
            max_crate_size: 10 * 1000,
            max_db_connections: 0,
            auth_required: LegacyAuthRequiredWrapper::Current(AuthRequired::No),
            required_crate_fields: Vec::new(),
            new_crates_restricted: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Copy)]
#[serde(untagged)]
pub enum LegacyAuthRequiredWrapper {
    //#[deprecated(note = "Use the new enum variant instead")]
    Deprecated(bool),
    Current(AuthRequired),
}

impl LegacyAuthRequiredWrapper {
    pub fn for_api(self) -> bool {
        match self {
            Self::Deprecated(value) => value,
            Self::Current(value) => value.for_api(),
        }
    }

    pub fn for_ui(self) -> bool {
        match self {
            Self::Deprecated(value) => value,
            Self::Current(value) => value.for_ui(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Copy)]
pub enum AuthRequired {
    No,
    All,
    OnlyApi,
}

impl From<LegacyAuthRequiredWrapper> for AuthRequired {
    fn from(value: LegacyAuthRequiredWrapper) -> Self {
        match value {
            LegacyAuthRequiredWrapper::Deprecated(true) => Self::All,
            LegacyAuthRequiredWrapper::Deprecated(false) => Self::No,
            LegacyAuthRequiredWrapper::Current(current) => current,
        }
    }
}

impl AuthRequired {
    pub fn for_api(self) -> bool {
        match self {
            Self::No => false,
            Self::All | Self::OnlyApi => true,
        }
    }

    pub fn for_ui(self) -> bool {
        match self {
            Self::No | Self::OnlyApi => false,
            Self::All => true,
        }
    }
}
