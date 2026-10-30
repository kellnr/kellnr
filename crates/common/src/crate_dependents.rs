use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::canonical_name::CanonicalName;
use crate::crate_data::is_outdated;
use crate::index_metadata::IndexDep;

/// Crates in the registry whose latest version depends on a crate.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateDependents {
    // Latest version of the crate that is not yanked, None if there is none.
    pub latest_version: Option<String>,
    // Dependents the caller may see, sorted by name.
    pub dependents: Vec<CrateDependent>,
    // Number of dependents the caller cannot access. Only the number is
    // shown, nothing else about them.
    pub restricted: u64,
}

/// A crate whose latest version depends on another crate.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateDependent {
    pub name: String,
    // Latest version of the dependent that is not yanked
    pub version: String,
    // How the dependent depends on the crate. Usually one entry, more if it
    // uses the crate e.g. as normal and as dev dependency.
    pub requirements: Vec<DependentRequirement>,
    // Whether any requirement does not allow the latest version of the crate
    pub outdated: bool,
}

#[derive(
    Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
pub struct DependentRequirement {
    // The semver requirement, e.g. "^1.2"
    pub version_req: String,
    // "normal", "build" or "dev"
    pub kind: String,
    // Whether the latest version of the crate is newer than the requirement allows
    pub outdated: bool,
}

/// A dependency on a crate of the same registry. These are stored for every
/// crate version to find the dependents of a crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDependency {
    // Package name, the original one for renamed dependencies
    pub name: CanonicalName,
    pub version_req: String,
    // "normal", "build" or "dev"
    pub kind: String,
}

impl LocalDependency {
    /// `None` for dependencies from other registries, like crates.io, and for
    /// names no crate can have.
    pub fn from_index_dep(dep: &IndexDep) -> Option<Self> {
        if dep.registry.is_some() {
            return None;
        }
        Some(LocalDependency {
            name: CanonicalName::try_from(dep.package.as_deref().unwrap_or(&dep.name)).ok()?,
            version_req: dep.req.clone(),
            kind: dep
                .kind
                .as_ref()
                .map_or_else(|| "normal".to_string(), ToString::to_string),
        })
    }
}

impl CrateDependent {
    /// The dependent `name` in `version`, which depends on a crate with the
    /// `(version_req, kind)` pairs in `requirements`. `latest_version` is the
    /// latest version of that crate.
    pub fn new(
        name: String,
        version: String,
        requirements: impl IntoIterator<Item = (String, String)>,
        latest_version: Option<&str>,
    ) -> Self {
        let mut requirements: Vec<DependentRequirement> = requirements
            .into_iter()
            .map(|(version_req, kind)| DependentRequirement {
                outdated: latest_version.is_some_and(|latest| is_outdated(&version_req, latest)),
                version_req,
                kind,
            })
            .collect();
        // Target specific dependencies can repeat the same requirement
        requirements.sort();
        requirements.dedup();
        CrateDependent {
            name,
            version,
            outdated: requirements.iter().any(|r| r.outdated),
            requirements,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_metadata::DependencyKind;

    fn dep(name: &str, req: &str, kind: Option<DependencyKind>) -> IndexDep {
        IndexDep {
            name: name.to_string(),
            req: req.to_string(),
            features: vec![],
            optional: false,
            default_features: true,
            target: None,
            kind,
            registry: None,
            package: None,
        }
    }

    fn dependent(requirements: &[(&str, &str)], latest: Option<&str>) -> CrateDependent {
        CrateDependent::new(
            "bar".to_string(),
            "1.0.0".to_string(),
            requirements
                .iter()
                .map(|(req, kind)| ((*req).to_string(), (*kind).to_string())),
            latest,
        )
    }

    #[test]
    fn dependent_on_old_major_is_outdated() {
        let dependent = dependent(&[("^1", "normal")], Some("2.0.0"));
        assert_eq!("bar", dependent.name);
        assert_eq!("1.0.0", dependent.version);
        assert!(dependent.outdated);
        assert_eq!(
            vec![DependentRequirement {
                version_req: "^1".to_string(),
                kind: "normal".to_string(),
                outdated: true,
            }],
            dependent.requirements
        );
    }

    #[test]
    fn dependent_on_latest_is_not_outdated() {
        let dependent = dependent(&[("^2", "normal")], Some("2.0.0"));
        assert!(!dependent.outdated);
    }

    #[test]
    fn dependent_is_outdated_if_any_requirement_is() {
        let dependent = dependent(
            &[("^2", "normal"), ("^1", "dev"), ("^1", "dev")],
            Some("2.0.0"),
        );
        assert!(dependent.outdated);
        assert_eq!(2, dependent.requirements.len());
        assert_eq!("dev", dependent.requirements[0].kind);
        assert!(!dependent.requirements[1].outdated);
    }

    #[test]
    fn without_latest_version_nothing_is_outdated() {
        assert!(!dependent(&[("^1", "normal")], None).outdated);
    }

    #[test]
    fn local_dependency_uses_canonical_package_name() {
        let mut renamed = dep("foo_renamed", "^2", Some(DependencyKind::Dev));
        renamed.package = Some("Foo-Bar".to_string());
        assert_eq!(
            Some(LocalDependency {
                name: CanonicalName::try_from("foo_bar").unwrap(),
                version_req: "^2".to_string(),
                kind: "dev".to_string(),
            }),
            LocalDependency::from_index_dep(&renamed)
        );
    }

    #[test]
    fn local_dependency_without_kind_is_normal() {
        let local = LocalDependency::from_index_dep(&dep("foo", "^1", None)).unwrap();
        assert_eq!("normal", local.kind);
    }

    #[test]
    fn dependency_from_other_registry_is_not_local() {
        let mut cratesio = dep("foo", "^1", None);
        cratesio.registry = Some("https://github.com/rust-lang/crates.io-index".to_string());
        assert_eq!(None, LocalDependency::from_index_dep(&cratesio));
    }
}
