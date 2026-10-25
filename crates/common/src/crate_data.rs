use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::index_metadata::IndexDep;
use crate::publish_metadata::RegistryDep;

/// Index URL that cargo sets as the registry of crates.io dependencies
pub const CRATESIO_INDEX: &str = "https://github.com/rust-lang/crates.io-index";

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
    // Size of the .crate file in bytes, 0 if unknown
    pub size: i64,
    // Minimal supported Rust version
    pub rust_version: Option<String>,
    // User who published the version, None if unknown
    pub publisher: Option<String>,
    // Name of the auth token used to publish, None if unknown
    pub publisher_token: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
    // Latest version of the dependency that is not yanked, if Kellnr knows
    // it. crates.io dependencies are only known if they are in the proxy cache.
    pub latest_version: Option<String>,
    // Whether the latest version is newer than the version requirement allows.
    pub outdated: bool,
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
            latest_version: None,
            outdated: false,
        }
    }
}

impl CrateRegistryDep {
    pub fn from_index(desc: Option<String>, latest_version: Option<String>, dep: IndexDep) -> Self {
        let outdated = latest_version
            .as_deref()
            .is_some_and(|latest| is_outdated(&dep.req, latest));
        CrateRegistryDep {
            name: match dep.package {
                Some(ref package) => package.clone(),
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
            latest_version,
            outdated,
        }
    }
}

/// Whether `latest` is newer than the version requirement `req` allows, e.g.
/// `^2.0` with the latest version `3.1`. A requirement that is newer than the
/// latest version, or that cannot be parsed, is not outdated.
pub fn is_outdated(req: &str, latest: &str) -> bool {
    let (Ok(req), Ok(latest)) = (
        semver::VersionReq::parse(req),
        semver::Version::parse(latest),
    ) else {
        return false;
    };
    if req.matches(&latest) {
        return false;
    }
    // Not outdated if the latest version is below a lower bound of the
    // requirement. Upper bounds like `<2` are what make a version outdated.
    let below_lower_bound = req.comparators.iter().any(|c| {
        let bound = semver::Version::new(c.major, c.minor.unwrap_or(0), c.patch.unwrap_or(0));
        match c.op {
            semver::Op::Less | semver::Op::LessEq => false,
            semver::Op::Greater => latest <= bound,
            _ => latest < bound,
        }
    });
    !below_lower_bound
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_version_outside_the_requirement_is_outdated() {
        assert!(is_outdated("^2.0", "3.1.0"));
        assert!(is_outdated("2.0", "3.1.0"));
        assert!(is_outdated("~1.2", "1.3.0"));
        assert!(is_outdated("=1.0.0", "1.0.1"));
        assert!(is_outdated(">=1, <2", "2.0.0"));
        assert!(is_outdated("^0.3", "0.4.0"));
        assert!(is_outdated("<2", "3.0.0"));
    }

    #[test]
    fn latest_version_inside_the_requirement_is_not_outdated() {
        assert!(!is_outdated("^2.0", "2.4.0"));
        assert!(!is_outdated("*", "9.0.0"));
        assert!(!is_outdated(">=1", "3.0.0"));
    }

    #[test]
    fn requirement_newer_than_the_latest_version_is_not_outdated() {
        assert!(!is_outdated("^5", "4.0.0"));
        assert!(!is_outdated(">=2.1", "2.0.0"));
        assert!(!is_outdated(">2.0.0", "2.0.0"));
        assert!(!is_outdated("=2.0.0", "1.0.0"));
    }

    #[test]
    fn unparsable_versions_are_not_outdated() {
        assert!(!is_outdated("not a req", "1.0.0"));
        assert!(!is_outdated("^1", "latest"));
    }
}
