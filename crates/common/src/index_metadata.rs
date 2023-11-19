use crate::{
    publish_metadata::{PublishMetadata, RegistryDep},
    version::Version,
};
use anyhow::anyhow;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter, Write};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

// This Metadata struct defined here is the one saved in the index.
// It is different to the one send by Cargo to the registry.
// See: https://doc.rust-lang.org/cargo/reference/registries.html#index-format
// Crates.io implementation: https://github.com/rust-lang/crates.io/blob/master/cargo-registry-index/lib.rs

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct IndexMetadata {
    // The name of the package.
    // This must only contain alphanumeric, `-`, or `_` characters.
    pub name: String,
    // The version of the package this row is describing.
    // This must be a valid version number according to the Semantic
    // Versioning 2.0.0 spec at https://semver.org/.
    pub vers: String,
    // Array of direct dependencies of the package.
    pub deps: Vec<IndexDep>,
    // A SHA256 checksum of the `.crate` file.
    pub cksum: String,
    // Set of features defined for the package.
    // Each feature maps to an array of features or dependencies it enables.
    // #[serde(
    //     skip_serializing_if = "Option::is_none",
    //     serialize_with = "option_sorted_map"
    // )]
    pub features: BTreeMap<String, Vec<String>>,
    // Boolean of whether or not this version has been yanked.
    pub yanked: bool,
    // The `links` string value from the package's manifest, or null if not
    // specified. This field is optional and defaults to null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<String>,
    // An unsigned 32-bit integer value indicating the schema version of this
    // entry.
    //
    // If this not specified, it should be interpreted as the default of 1.
    //
    // Cargo (starting with version 1.51) will ignore versions it does not
    // recognize. This provides a method to safely introduce changes to index
    // entries and allow older versions of cargo to ignore newer entries it
    // doesn't understand. Versions older than 1.51 ignore this field, and
    // thus may misinterpret the meaning of the index entry.
    //
    // The current values are:
    //
    // * 1: The schema as documented here, not including newer additions.
    //      This is honored in Rust version 1.51 and newer.
    // * 2: The addition of the `features2` field.
    //      This is honored in Rust version 1.60 and newer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v: Option<u32>,
    // This optional field contains features with new, extended syntax.
    // Specifically, namespaced features (`dep:`) and weak dependencies
    // (`pkg?/feat`).
    //
    // This is separated from `features` because versions older than 1.19
    // will fail to load due to not being able to parse the new syntax, even
    // with a `Cargo.lock` file.
    //
    // Cargo will merge any values listed here with the "features" field.
    //
    // If this field is included, the "v" field should be set to at least 2.
    //
    // Registries are not required to use this field for extended feature
    // syntax, they are allowed to include those in the "features" field.
    // Using this is only necessary if the registry wants to support cargo
    // versions older than 1.19, which in practice is only crates.io since
    // those older versions do not support other registries.
    // "features2": {
    // "serde": ["dep:serde", "chrono?/serde"]
    // }
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features2: Option<BTreeMap<String, Vec<String>>>,
}

impl IndexMetadata {
    pub async fn from_max_version(path: &Path) -> Result<Self, anyhow::Error> {
        let mut file = File::open(path).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;

        let mut metadata: Vec<IndexMetadata> = content
            .lines()
            .filter_map(|m| serde_json::from_str::<IndexMetadata>(m).ok())
            .collect();

        metadata.sort_by(|a, b| {
            let sv1 = Version::try_from(&a.vers).unwrap();
            let sv2 = Version::try_from(&b.vers).unwrap();
            sv1.cmp(&sv2)
        });

        metadata
            .last()
            .cloned()
            .ok_or_else(|| anyhow!("Unable to read metadata file."))
    }

    pub async fn from_version(path: &Path, version: &Version) -> Result<Self, anyhow::Error> {
        let mut file = File::open(path).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;

        let metadata: Vec<IndexMetadata> = content
            .lines()
            .filter_map(|m| serde_json::from_str::<IndexMetadata>(m).ok())
            .collect();

        metadata
            .iter()
            .find(|m| {
                let sv = Version::try_from(&m.vers).unwrap_or_default();
                sv == *version
            })
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "Unable to read metadata for version {} from file {}.",
                    version,
                    path.display()
                )
            })
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }

    pub fn metadata_path(&self, index_path: &Path) -> PathBuf {
        metadata_path(index_path, &self.name)
    }

    pub fn from_reg_meta(registry_metadata: &PublishMetadata, cksum: &str) -> Self {
        IndexMetadata {
            name: registry_metadata.name.clone(),
            vers: registry_metadata.vers.clone(),
            deps: registry_metadata
                .deps
                .clone()
                .into_iter()
                .map(IndexDep::from)
                .collect(),
            cksum: cksum.to_string(),
            features: registry_metadata.features.clone(),
            yanked: false,
            links: registry_metadata.links.clone(),
            v: Some(1),
            features2: None,
        }
    }

    pub fn minimal(name: &str, vers: &str, cksum: &str) -> Self {
        Self {
            name: name.to_string(),
            vers: vers.to_string(),
            cksum: cksum.to_string(),
            deps: vec![],
            features: Default::default(),
            yanked: false,
            links: None,
            v: Some(1),
            features2: None,
        }
    }

    pub fn serialize_indices(indices: &[IndexMetadata]) -> Result<String, serde_json::Error> {
        let indices = indices
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, serde_json::Error>>()?;
        let mut index = String::new();
        for (i, ix) in indices.iter().enumerate() {
            if i == indices.len() - 1 {
                write!(&mut index, "{}", ix).unwrap();
            } else {
                writeln!(&mut index, "{}", ix).unwrap();
            }
        }
        Ok(index)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct IndexDep {
    // Name of the dependency.
    // If the dependency is renamed from the original package name,
    // this is the new name. The original package name is stored in
    // the `package` field.
    pub name: String,
    // The SemVer requirement for this dependency.
    // This must be a valid version requirement defined at
    // https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html.
    pub req: String,
    // Array of features (as strings) enabled for this dependency.
    pub features: Vec<String>,
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
    // Note: this is a required field, but a small number of entries
    // exist in the crates.io index with either a missing or null
    // `kind` field due to implementation bugs.
    pub kind: Option<DependencyKind>,
    // The URL of the index of the registry where this dependency is
    // from as a string. If not specified or null, it is assumed the
    // dependency is in the current registry.
    pub registry: Option<String>,
    // If the dependency is renamed, this is a string of the actual
    // package name. If not specified or null, this dependency is not
    // renamed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub enum DependencyKind {
    Normal,
    Build,
    Dev,
    Other(String),
}

impl<'de> Deserialize<'de> for DependencyKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "normal" => Ok(DependencyKind::Normal),
            "build" => Ok(DependencyKind::Build),
            "dev" => Ok(DependencyKind::Dev),
            _ => Ok(DependencyKind::Other(s)),
        }
    }
}

impl Serialize for DependencyKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            DependencyKind::Normal => serializer.serialize_str("normal"),
            DependencyKind::Build => serializer.serialize_str("build"),
            DependencyKind::Dev => serializer.serialize_str("dev"),
            DependencyKind::Other(s) => serializer.serialize_str(s),
        }
    }
}

impl Display for DependencyKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyKind::Normal => write!(f, "normal"),
            DependencyKind::Build => write!(f, "build"),
            DependencyKind::Dev => write!(f, "dev"),
            DependencyKind::Other(s) => write!(f, "{}", s),
        }
    }
}

impl From<String> for DependencyKind {
    fn from(kind: String) -> Self {
        match kind.as_str() {
            "normal" => DependencyKind::Normal,
            "build" => DependencyKind::Build,
            "dev" => DependencyKind::Dev,
            _ => DependencyKind::Other(kind),
        }
    }
}

impl From<RegistryDep> for IndexDep {
    fn from(registry_dep: RegistryDep) -> Self {
        IndexDep {
            name: match registry_dep.explicit_name_in_toml {
                Some(ref name) => name.clone(),
                None => registry_dep.name.clone(),
            },
            req: registry_dep.version_req,
            features: registry_dep.features.unwrap_or_default(),
            optional: registry_dep.optional,
            default_features: registry_dep.default_features,
            target: registry_dep.target,
            kind: registry_dep.kind.map(DependencyKind::from),
            registry: registry_dep.registry,
            package: match registry_dep.explicit_name_in_toml {
                Some(_) => Some(registry_dep.name),
                None => None,
            },
        }
    }
}

pub fn metadata_path(index_path: &Path, name: &str) -> PathBuf {
    if name.len() == 1 {
        index_path.join("1").join(name.to_lowercase())
    } else if name.len() == 2 {
        index_path.join("2").join(name.to_lowercase())
    } else if name.len() == 3 {
        let first_char = &name[0..1].to_lowercase();
        index_path
            .join("3")
            .join(first_char)
            .join(name.to_lowercase())
    } else {
        let first_two = &name[0..2].to_lowercase();
        let second_two = &name[2..4].to_lowercase();
        index_path
            .join(first_two)
            .join(second_two)
            .join(name.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitive_dependency_rename() {
        let reg_meta = PublishMetadata {
            name: "foo".to_string(),
            vers: "0.1.0".to_string(),
            deps: vec![
                RegistryDep {
                    name: "bar".to_string(),
                    version_req: "^0.1.0".to_string(),
                    features: None,
                    optional: false,
                    default_features: true,
                    target: None,
                    kind: None,
                    registry: None,
                    explicit_name_in_toml: None,
                },
                RegistryDep {
                    name: "baz".to_string(),
                    version_req: "^0.1.0".to_string(),
                    features: None,
                    optional: false,
                    default_features: true,
                    target: None,
                    kind: None,
                    registry: None,
                    explicit_name_in_toml: Some("qux".to_string()),
                },
            ],
            features: Default::default(),
            links: None,
            description: None,
            authors: None,
            documentation: None,
            homepage: None,
            readme: None,
            readme_file: None,
            keywords: Default::default(),
            categories: Default::default(),
            license: None,
            license_file: None,
            repository: None,
            badges: None,
        };

        let index_meta = IndexMetadata::from_reg_meta(&reg_meta, "1234");

        assert_eq!(index_meta.deps.len(), 2);
        assert_eq!(index_meta.deps[0].name, "bar");
        assert_eq!(index_meta.deps[0].package, None);
        assert_eq!(index_meta.deps[1].name, "qux");
        assert_eq!(index_meta.deps[1].package, Some("baz".to_string()));
    }

    #[test]
    fn metadata_path_one_letter() {
        let name = "A";
        assert_eq!(
            metadata_path(&PathBuf::from("ip"), name),
            Path::new("ip").join("1").join("a")
        );
    }

    #[test]
    fn metadata_path_two_letters() {
        let name = "cB";
        assert_eq!(
            metadata_path(&PathBuf::from("ip"), name),
            Path::new("ip").join("2").join("cb")
        );
    }

    #[test]
    fn metadata_path_three_letters() {
        let name = "cAb";
        assert_eq!(
            metadata_path(&PathBuf::from("ip"), name),
            Path::new("ip").join("3").join("c").join("cab")
        );
    }

    #[test]
    fn metadata_path_four_or_more_letters() {
        let name = "foo_bAr";
        assert_eq!(
            metadata_path(&PathBuf::from("ip"), name),
            Path::new("ip").join("fo").join("o_").join("foo_bar")
        );
    }
}
