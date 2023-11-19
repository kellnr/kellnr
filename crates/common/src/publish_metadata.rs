use crate::index_metadata::IndexDep;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

// The Metadata struct defined here is the one send by Cargo to the registry.
// It is different to the one saved in the index!
// See: https://doc.rust-lang.org/cargo/reference/registries.html#publish
// Crates.io implementation: https://github.com/rust-lang/crates.io/blob/5c7070829418f72639dd0b91993eaf91e516416b/src/views/krate_publish.rs

#[derive(Default, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct PublishMetadata {
    // The name of the package.
    pub name: String,
    // The version of the package being published.
    pub vers: String,
    // Array of direct dependencies of the package.
    pub deps: Vec<RegistryDep>,
    // Set of features defined for the package.
    // Each feature maps to an array of features or dependencies it enables.
    // Cargo does not impose limitations on feature names, but crates.io
    // requires alphanumeric ASCII, `_` or `-` characters.
    pub features: BTreeMap<String, Vec<String>>,
    // List of strings of the authors.
    // May be empty.
    pub authors: Option<Vec<String>>,
    // Description field from the manifest.
    // May be null. crates.io requires at least some content.
    pub description: Option<String>,
    // String of the URL to the website for this package's documentation.
    // May be null.
    pub documentation: Option<String>,
    // String of the URL to the website for this package's home page.
    // May be null.
    pub homepage: Option<String>,
    // String of the content of the README file.
    // May be null.
    pub readme: Option<String>,
    // String of a relative path to a README file in the crate.
    // May be null.
    pub readme_file: Option<String>,
    // Array of strings of keywords for the package.
    #[serde(default)]
    pub keywords: Vec<String>,
    // Array of strings of categories for the package.
    #[serde(default)]
    pub categories: Vec<String>,
    // String of the license for the package.
    // May be null. crates.io requires either `license` or `license_file` to be set.
    pub license: Option<String>,
    // String of a relative path to a license file in the crate.
    // May be null.
    pub license_file: Option<String>,
    // String of the URL to the website for the source repository of this package.
    // May be null.
    pub repository: Option<String>,
    // Optional object of "status" badges. Each value is an object of
    // arbitrary string to string mappings.
    // crates.io has special interpretation of the format of the badges.
    pub badges: Option<HashMap<String, Option<HashMap<String, String>>>>,
    // The `links` string value from the package's manifest, or null if not
    // specified. This field is optional and defaults to null.
    #[serde(default)]
    pub links: Option<String>,
}

impl PublishMetadata {
    pub fn minimal(name: &str, vers: &str) -> Self {
        Self {
            name: name.to_string(),
            vers: vers.to_string(),
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RegistryDep {
    // Name of the dependency.
    // If the dependency is renamed from the original package name,
    // this is the original name. The new package name is stored in
    // the `explicit_name_in_toml` field.
    pub name: String,
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

impl From<IndexDep> for RegistryDep {
    fn from(dep: IndexDep) -> Self {
        RegistryDep {
            name: match dep.package {
                Some(ref package) => package.to_string(),
                None => dep.name.clone(),
            },
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
