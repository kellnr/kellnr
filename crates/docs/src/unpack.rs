use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use tar::{Archive, EntryType};
use tracing::warn;

use crate::docs_error::DocsError;

/// Uncompressed bytes an archive may extract to, per MB of `docs.max_size`.
const MAX_BYTES_PER_DOCS_MB: u64 = 20 * 1_000_000;
/// Entries an archive may contain, per MB of `docs.max_size`.
const MAX_ENTRIES_PER_DOCS_MB: usize = 1_000;

/// Upper bounds for extracting an archive, protecting against archive bombs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractLimits {
    max_bytes: u64,
    max_entries: usize,
}

impl ExtractLimits {
    pub fn new(max_bytes: u64, max_entries: usize) -> Self {
        Self {
            max_bytes,
            max_entries,
        }
    }

    /// Limits derived from the `docs.max_size` setting (in MB), which caps the
    /// compressed size of uploaded docs archives.
    pub fn from_max_docs_size(max_size_mb: usize) -> Self {
        let max_size_mb = max_size_mb.max(1);
        Self::new(
            MAX_BYTES_PER_DOCS_MB.saturating_mul(max_size_mb as u64),
            MAX_ENTRIES_PER_DOCS_MB.saturating_mul(max_size_mb),
        )
    }

    pub(crate) fn budget(self) -> ExtractBudget {
        ExtractBudget {
            remaining_bytes: self.max_bytes,
            remaining_entries: self.max_entries,
        }
    }
}

/// Remaining bytes and entries while extracting a single archive.
pub(crate) struct ExtractBudget {
    remaining_bytes: u64,
    remaining_entries: usize,
}

impl ExtractBudget {
    /// Account for the next archive entry.
    pub(crate) fn take_entry(&mut self) -> Result<(), DocsError> {
        self.remaining_entries = self
            .remaining_entries
            .checked_sub(1)
            .ok_or(DocsError::ArchiveTooLarge)?;
        Ok(())
    }

    /// Write `content` to a new regular file at `target`, creating parent
    /// directories. Fails once the archive exceeds its byte budget.
    pub(crate) fn write_file(
        &mut self,
        content: &mut impl Read,
        target: &Path,
    ) -> Result<(), DocsError> {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(target)?;
        let written = io::copy(
            &mut content.take(self.remaining_bytes.saturating_add(1)),
            &mut file,
        )?;
        self.remaining_bytes = self
            .remaining_bytes
            .checked_sub(written)
            .ok_or(DocsError::ArchiveTooLarge)?;
        Ok(())
    }
}

/// Unpack a `.crate` archive into `dest`.
///
/// Only regular files and directories below `root` (the `{name}-{version}`
/// folder every crate archive contains) are written. Symlinks, hard links and
/// device files are skipped, so a crafted archive cannot point later steps,
/// like the docs upload, at files outside of `dest`.
pub fn unpack_crate(
    reader: impl Read,
    dest: &Path,
    root: &str,
    limits: ExtractLimits,
) -> Result<(), DocsError> {
    let mut budget = limits.budget();
    let mut archive = Archive::new(reader);
    for entry in archive.entries()? {
        let mut entry = entry?;
        budget.take_entry()?;
        let path = entry.path()?.into_owned();
        let Some(relative) = path_below_root(&path, root) else {
            warn!(
                "Skipping crate archive entry outside of {root}: {}",
                path.display()
            );
            continue;
        };
        let target = dest.join(relative);
        match entry.header().entry_type() {
            EntryType::Directory => fs::create_dir_all(&target)?,
            EntryType::Regular | EntryType::Continuous => {
                budget.write_file(&mut entry, &target)?;
            }
            other => warn!(
                "Skipping crate archive entry {} of type {other:?}",
                path.display()
            ),
        }
    }
    Ok(())
}

/// Returns `path` if it only consists of plain components and starts with `root`.
fn path_below_root(path: &Path, root: &str) -> Option<PathBuf> {
    let mut components = path.components();
    if components.next() != Some(Component::Normal(root.as_ref())) {
        return None;
    }
    components
        .all(|c| matches!(c, Component::Normal(_)))
        .then(|| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use tar::{Builder, Header};

    use super::*;

    fn tar_header(path: &str, entry_type: EntryType, size: u64) -> Header {
        let mut header = Header::new_gnu();
        // Set the raw name so tests can use paths `Builder` itself would reject.
        header.as_gnu_mut().unwrap().name[..path.len()].copy_from_slice(path.as_bytes());
        header.set_entry_type(entry_type);
        header.set_size(size);
        header.set_mode(0o644);
        header.set_cksum();
        header
    }

    fn append_file(builder: &mut Builder<Vec<u8>>, path: &str, content: &[u8]) {
        let header = tar_header(path, EntryType::Regular, content.len() as u64);
        builder.append(&header, content).unwrap();
    }

    fn append_link(builder: &mut Builder<Vec<u8>>, path: &str, kind: EntryType, target: &str) {
        let mut header = tar_header(path, kind, 0);
        header.set_link_name(target).unwrap();
        header.set_cksum();
        builder.append(&header, io::empty()).unwrap();
    }

    const LIMITS: ExtractLimits = ExtractLimits {
        max_bytes: 1_000,
        max_entries: 10,
    };

    fn try_unpack(
        limits: ExtractLimits,
        build: impl FnOnce(&mut Builder<Vec<u8>>),
    ) -> (tempfile::TempDir, Result<(), DocsError>) {
        let mut builder = Builder::new(Vec::new());
        build(&mut builder);
        let tar = builder.into_inner().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let result = unpack_crate(tar.as_slice(), dest.path(), "x-1.0.0", limits);
        (dest, result)
    }

    fn unpack(build: impl FnOnce(&mut Builder<Vec<u8>>)) -> tempfile::TempDir {
        let (dest, result) = try_unpack(LIMITS, build);
        result.unwrap();
        dest
    }

    #[test]
    fn unpack_crate_fails_when_uncompressed_size_exceeds_budget() {
        let (_dest, result) = try_unpack(LIMITS, |b| {
            append_file(b, "x-1.0.0/a", &[0; 600]);
            append_file(b, "x-1.0.0/b", &[0; 600]);
        });

        assert!(matches!(result, Err(DocsError::ArchiveTooLarge)));
    }

    #[test]
    fn unpack_crate_accepts_archive_of_exactly_the_budget() {
        let (_dest, result) = try_unpack(LIMITS, |b| {
            append_file(b, "x-1.0.0/a", &[0; 500]);
            append_file(b, "x-1.0.0/b", &[0; 500]);
        });

        assert!(result.is_ok());
    }

    #[test]
    fn unpack_crate_fails_when_entry_count_exceeds_budget() {
        let (_dest, result) = try_unpack(LIMITS, |b| {
            for i in 0..11 {
                append_file(b, &format!("x-1.0.0/{i}"), b"");
            }
        });

        assert!(matches!(result, Err(DocsError::ArchiveTooLarge)));
    }

    #[test]
    fn extract_limits_scale_with_max_docs_size() {
        assert_eq!(
            ExtractLimits::from_max_docs_size(100),
            ExtractLimits::new(2_000_000_000, 100_000)
        );
        assert_eq!(
            ExtractLimits::from_max_docs_size(0),
            ExtractLimits::from_max_docs_size(1)
        );
    }

    #[test]
    fn unpack_crate_writes_files_and_directories() {
        let dest = unpack(|b| {
            let dir = tar_header("x-1.0.0/src", EntryType::Directory, 0);
            b.append(&dir, io::empty()).unwrap();
            append_file(b, "x-1.0.0/Cargo.toml", b"[package]");
            append_file(b, "x-1.0.0/src/lib.rs", b"pub fn f() {}");
        });

        let root = dest.path().join("x-1.0.0");
        assert_eq!(fs::read(root.join("Cargo.toml")).unwrap(), b"[package]");
        assert_eq!(fs::read(root.join("src/lib.rs")).unwrap(), b"pub fn f() {}");
    }

    #[test]
    fn unpack_crate_skips_symlinks_and_hardlinks() {
        let secret = tempfile::NamedTempFile::new().unwrap();
        fs::write(secret.path(), b"secret").unwrap();
        let target = secret.path().to_str().unwrap();

        let dest = unpack(|b| {
            append_link(b, "x-1.0.0/symlink.html", EntryType::Symlink, target);
            append_link(b, "x-1.0.0/hardlink.html", EntryType::Link, target);
            append_file(b, "x-1.0.0/Cargo.toml", b"[package]");
        });

        let root = dest.path().join("x-1.0.0");
        assert!(fs::symlink_metadata(root.join("symlink.html")).is_err());
        assert!(fs::symlink_metadata(root.join("hardlink.html")).is_err());
        assert!(root.join("Cargo.toml").exists());
    }

    #[test]
    fn unpack_crate_skips_entries_outside_of_root() {
        let dest = unpack(|b| {
            append_file(b, "other/file", b"other");
            append_file(b, "x-1.0.0/../escape", b"escape");
            append_file(b, "/abs", b"abs");
            append_file(b, "x-1.0.0/Cargo.toml", b"[package]");
        });

        assert!(!dest.path().join("other").exists());
        assert!(!dest.path().join("escape").exists());
        assert!(dest.path().join("x-1.0.0/Cargo.toml").exists());
        let entries: Vec<_> = fs::read_dir(dest.path()).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }
}
