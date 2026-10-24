use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use tar::{Archive, EntryType};
use tracing::warn;

use crate::docs_error::DocsError;

/// Unpack a `.crate` archive into `dest`.
///
/// Only regular files and directories below `root` (the `{name}-{version}`
/// folder every crate archive contains) are written. Symlinks, hard links and
/// device files are skipped, so a crafted archive cannot point later steps,
/// like the docs upload, at files outside of `dest`.
pub fn unpack_crate(reader: impl Read, dest: &Path, root: &str) -> Result<(), DocsError> {
    let mut archive = Archive::new(reader);
    for entry in archive.entries()? {
        let mut entry = entry?;
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
            EntryType::Regular | EntryType::Continuous => write_file(&mut entry, &target)?,
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

/// Write `content` to a new regular file at `target`, creating parent directories.
pub(crate) fn write_file(content: &mut impl Read, target: &Path) -> io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(target)?;
    io::copy(content, &mut file)?;
    Ok(())
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

    fn unpack(build: impl FnOnce(&mut Builder<Vec<u8>>)) -> tempfile::TempDir {
        let mut builder = Builder::new(Vec::new());
        build(&mut builder);
        let tar = builder.into_inner().unwrap();
        let dest = tempfile::tempdir().unwrap();
        unpack_crate(tar.as_slice(), dest.path(), "x-1.0.0").unwrap();
        dest
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
