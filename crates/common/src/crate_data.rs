use crate::index_metadata::IndexDep;
use crate::publish_metadata::RegistryDep;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateData {
    pub name: String,
    // additional information from kellnr about the crate
    pub owners: Vec<String>,
    pub max_version: String,
    pub total_downloads: i64,
    pub last_updated: String,
    // metadata information from the publishing
    pub homepage: Option<String>,
    pub description: Option<String>,
    pub repository: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub authors: Vec<String>,
    pub versions: Vec<CrateVersionData>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateVersionData {
    pub version: String,
    // additional information about the crate version from kellnr
    pub created: String,
    pub downloads: i64,
    // metadata information from the publishing
    pub readme: Option<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub documentation: Option<String>,
    pub dependencies: Vec<CrateRegistryDep>,
    pub checksum: String,
    pub features: BTreeMap<String, Vec<String>>,
    pub yanked: bool,
    pub links: Option<String>,
    pub v: i32,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateRegistryDep {
    // Name of the dependency.
    // If the dependency is renamed from the original package name,
    // this is the original name. The new package name is stored in
    // the `explicit_name_in_toml` field.
    pub name: String,
    pub description: Option<String>,
    // The semver requirement for this dependency.
    pub version_req: String,
    // Array of features (as strings) enabled for this dependency.
    pub features: Option<Vec<String>>,
    // Boolean of whether or not this is an optional dependency.
    pub optional: bool,
    // Boolean of whether or not default features are enabled.
    pub default_features: bool,
    // The target platform for the dependency.
    // null if not a target dependency.
    // Otherwise, a string such as "cfg(windows)".
    pub target: Option<String>,
    // The dependency kind.
    // "dev", "build", or "normal".
    pub kind: Option<String>,
    // The URL of the index of the registry where this dependency is
    // from as a string. If not specified or null, it is assumed the
    // dependency is in the current registry.
    pub registry: Option<String>,
    // If the dependency is renamed, this is a string of the new
    // package name. If not specified or null, this dependency is not
    // renamed.
    pub explicit_name_in_toml: Option<String>,
}

impl From<RegistryDep> for CrateRegistryDep {
    fn from(dep: RegistryDep) -> Self {
        CrateRegistryDep {
            name: dep.name,
            description: None,
            version_req: dep.version_req,
            features: dep.features,
            optional: dep.optional,
            default_features: dep.default_features,
            target: dep.target,
            kind: dep.kind,
            registry: dep.registry,
            explicit_name_in_toml: dep.explicit_name_in_toml,
        }
    }
}

impl CrateRegistryDep {
    pub fn from_index(desc: Option<String>, dep: IndexDep) -> Self {
        CrateRegistryDep {
            name: match dep.package {
                Some(ref package) => package.to_string(),
                None => dep.name.clone(),
            },
            description: desc,
            version_req: dep.req,
            features: Some(dep.features),
            optional: dep.optional,
            default_features: dep.default_features,
            target: dep.target,
            kind: dep.kind.map(|k| k.to_string()),
            registry: dep.registry,
            explicit_name_in_toml: match dep.package {
                Some(_) => Some(dep.name),
                None => None,
            },
        }
    }
}
