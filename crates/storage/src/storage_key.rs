use std::fmt;

use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;

use crate::docs_storage::DocsName;
use crate::toolchain_storage::ToolchainArchiveId;

/// The key of an object in a storage. Keys are stored paths, so the format
/// of every key must stay the same.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StorageKey(String);

impl StorageKey {
    /// The crate file, e.g. `my-crate-1.0.0.crate`.
    pub fn crate_file(name: &OriginalName, version: &Version) -> Self {
        Self(format!("{name}-{version}.crate"))
    }

    /// A file under the docs of a crate version, e.g.
    /// `my-crate/1.0.0/doc/my_crate/index.html`.
    pub fn docs_file(name: &DocsName, version: &Version, relative_path: &str) -> Self {
        Self(format!("{name}/{version}/{relative_path}"))
    }

    /// The archive of a toolchain target, e.g.
    /// `2024-01-15/rust-1.75.0-x86_64-unknown-linux-gnu.tar.xz`.
    pub fn toolchain_archive(id: &ToolchainArchiveId<'_>) -> Self {
        Self::toolchain_component(id, id.name)
    }

    /// The archive of one component of a toolchain target, e.g.
    /// `2024-01-15/rustc-1.75.0-x86_64-unknown-linux-gnu.tar.xz`.
    pub fn toolchain_component(id: &ToolchainArchiveId<'_>, component: &str) -> Self {
        Self::toolchain_file(
            id.date,
            &format!("{component}-{}-{}.tar.xz", id.version, id.target),
        )
    }

    /// A toolchain file by its release date and file name.
    pub fn toolchain_file(date: &str, file_name: &str) -> Self {
        Self(format!("{date}/{file_name}"))
    }

    /// A key as it was stored before, e.g. saved in the database, returned by
    /// a listing or requested by a client.
    pub fn from_stored(key: String) -> Self {
        Self(key)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for StorageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_keep_their_stored_format() {
        let name = OriginalName::try_from("My_Crate").unwrap();
        let version = Version::try_from("1.0.0-rc.1+build").unwrap();
        let toolchain = ToolchainArchiveId {
            name: "rust",
            version: "1.75.0",
            date: "2024-01-15",
            target: "x86_64-unknown-linux-gnu",
        };

        assert_eq!(
            "My_Crate-1.0.0-rc.1+build.crate",
            StorageKey::crate_file(&name, &version).as_str()
        );
        assert_eq!(
            "my_crate/1.0.0-rc.1+build/doc/my_crate/index.html",
            StorageKey::docs_file(&DocsName::from(&name), &version, "doc/my_crate/index.html")
                .as_str()
        );
        assert_eq!(
            "2024-01-15/rust-1.75.0-x86_64-unknown-linux-gnu.tar.xz",
            StorageKey::toolchain_archive(&toolchain).as_str()
        );
        assert_eq!(
            "2024-01-15/rustc-1.75.0-x86_64-unknown-linux-gnu.tar.xz",
            StorageKey::toolchain_component(&toolchain, "rustc").as_str()
        );
        assert_eq!(
            "2024-01-15/cargo.tar.gz",
            StorageKey::toolchain_file("2024-01-15", "cargo.tar.gz").as_str()
        );
    }
}
