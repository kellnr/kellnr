use std::path::{Path, PathBuf};
use std::sync::Arc;

use cargo::GlobalContext;
use cargo::core::Workspace;
use cargo::core::compiler::UserIntent;
use cargo::core::resolver::CliFeatures;
use cargo::ops::{self, CompileOptions, DocOptions, OutputFormat};
use flate2::read::GzDecoder;
use fs_extra::dir::{CopyOptions, copy};
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::{DbProvider, DocQueueEntry};
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use tar::Archive;
use tokio::fs::{create_dir_all, remove_dir_all};
use tracing::error;

use crate::compute_doc_url;
use crate::docs_error::DocsError;

pub fn doc_extraction_queue(
    db: Arc<dyn DbProvider>,
    cs: Arc<KellnrCrateStorage>,
    docs_path: PathBuf,
    path_prefix: String,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            if let Err(e) = inner_loop(db.clone(), &cs, &docs_path, &path_prefix).await {
                error!("Rustdoc generation loop failed: {e}");
            }
        }
    });
}

async fn inner_loop(
    db: Arc<dyn DbProvider>,
    cs: &KellnrCrateStorage,
    docs_path: &Path,
    path_prefix: &str,
) -> Result<(), DocsError> {
    let entries = db.get_doc_queue().await?;

    for entry in entries {
        if let Err(e) = extract_docs(&entry, cs, docs_path).await {
            error!("Failed to extract docs from crate: {e}");
        } else {
            if let Err(e) = clean_up(&entry.path).await {
                error!("Failed to delete temporary rustdoc queue folder: {e}");
            }

            let version = Version::from_unchecked_str(&entry.version);
            let docs_link = compute_doc_url(&entry.normalized_name, &version, path_prefix);
            db.update_docs_link(&entry.normalized_name, &version, &docs_link)
                .await?;
        }
        db.delete_doc_queue(entry.id).await?;
    }

    Ok(())
}

async fn extract_docs(
    doc: &DocQueueEntry,
    cs: &KellnrCrateStorage,
    docs_path: &Path,
) -> Result<(), DocsError> {
    // Unpack crate

    // TODO: Only works if normalized name = original name -> Need to get original name from db
    let orig_name = OriginalName::from_unchecked(doc.normalized_name.to_string());
    let version = Version::from_unchecked_str(&doc.version);
    let contents = cs.get(&orig_name, &version).await.ok_or_else(|| {
        error!("Failed to get crate from storage");
        DocsError::CrateDoesNotExist(doc.normalized_name.to_string(), doc.version.clone())
    })?;
    let tar = GzDecoder::new(std::io::Cursor::new(contents));
    let mut archive = Archive::new(tar);
    archive.unpack(&doc.path)?;

    // Generate the docs
    let generated_docs_path = &doc
        .path
        .join(format!("{}-{}", doc.normalized_name, doc.version));
    strip_rust_toolchain_files(generated_docs_path).await?;
    generate_docs(generated_docs_path)?;

    // Copy the docs directory
    let from = generated_docs_path.join("target").join("doc");
    let to = docs_path
        .join(doc.normalized_name.to_string())
        .join(&doc.version);
    copy_dir(&from, &to).await?;

    Ok(())
}

async fn clean_up(path: &Path) -> Result<(), DocsError> {
    remove_dir_all(path).await?;
    Ok(())
}

/// Remove any `rust-toolchain.toml` (or legacy `rust-toolchain`) at the crate
/// root before invoking cargo.
///
/// These files are a rustup feature for pinning a local development or CI
/// toolchain. They were never intended as a contract with downstream
/// consumers: the canonical way to declare a minimum supported Rust version
/// is `package.rust-version` in `Cargo.toml`, which cargo's resolver honors
/// without swapping out the compiler.
///
/// However, cargo does not exclude these files from `cargo publish` by
/// default, so some crates accidentally ship them. When that happens, the
/// rustup proxy that backs `rustc` inside the Kellnr container walks up from
/// the working directory, finds the file, and silently switches to the
/// pinned toolchain (downloading it if necessary). If the pin predates a
/// stable feature the crate now relies on (e.g. `check-cfg`), `cargo doc`
/// fails with errors that look like bugs in the crate or in Kellnr but are
/// really an invisible toolchain swap. See issue #1176.
///
/// Dropping the file here only covers the crate currently being documented.
/// Transitive dependencies that ship the same accident are unpacked by cargo
/// into its own registry cache and need `RUSTUP_TOOLCHAIN` set in the
/// container environment to neutralize.
async fn strip_rust_toolchain_files(crate_path: &Path) -> Result<(), DocsError> {
    for name in ["rust-toolchain.toml", "rust-toolchain"] {
        let path = crate_path.join(name);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

async fn copy_dir(from: &Path, to: &Path) -> Result<(), DocsError> {
    create_dir_all(to).await?;
    copy(
        from,
        to,
        &CopyOptions {
            overwrite: true,
            ..CopyOptions::default()
        },
    )?;
    Ok(())
}

fn generate_docs(crate_path: impl AsRef<Path>) -> Result<(), DocsError> {
    let manifest_path = crate_path.as_ref().join("Cargo.toml").canonicalize()?;
    let ctx = GlobalContext::default().map_err(|e| DocsError::CargoError(e.to_string()))?;
    let workspace =
        Workspace::new(&manifest_path, &ctx).map_err(|e| DocsError::CargoError(e.to_string()))?;
    let compile_opts = CompileOptions {
        cli_features: CliFeatures::new_all(true),
        ..CompileOptions::new(
            &ctx,
            UserIntent::Doc {
                deps: false,
                json: false,
            },
        )
        .map_err(|e| DocsError::CargoError(e.to_string()))?
    };
    let options = DocOptions {
        open_result: false,
        compile_opts,
        output_format: OutputFormat::Html,
    };
    ops::doc(&workspace, &options).map_err(|e| DocsError::CargoError(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn strip_rust_toolchain_files_removes_both_variants() {
        let dir = tempfile::tempdir().unwrap();
        let toml_path = dir.path().join("rust-toolchain.toml");
        let legacy_path = dir.path().join("rust-toolchain");
        tokio::fs::write(&toml_path, "[toolchain]\nchannel = \"1.65.0\"\n")
            .await
            .unwrap();
        tokio::fs::write(&legacy_path, "1.65.0\n").await.unwrap();

        strip_rust_toolchain_files(dir.path()).await.unwrap();

        assert!(!toml_path.exists());
        assert!(!legacy_path.exists());
    }

    #[tokio::test]
    async fn strip_rust_toolchain_files_noop_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let unrelated = dir.path().join("Cargo.toml");
        tokio::fs::write(&unrelated, "[package]\nname = \"x\"\n")
            .await
            .unwrap();

        strip_rust_toolchain_files(dir.path()).await.unwrap();

        assert!(unrelated.exists());
    }

    #[tokio::test]
    async fn strip_rust_toolchain_files_only_removes_named_files() {
        let dir = tempfile::tempdir().unwrap();
        let toml_path = dir.path().join("rust-toolchain.toml");
        let cargo_toml = dir.path().join("Cargo.toml");
        let src = dir.path().join("src");
        tokio::fs::write(&toml_path, "").await.unwrap();
        tokio::fs::write(&cargo_toml, "").await.unwrap();
        tokio::fs::create_dir(&src).await.unwrap();

        strip_rust_toolchain_files(dir.path()).await.unwrap();

        assert!(!toml_path.exists());
        assert!(cargo_toml.exists());
        assert!(src.exists());
    }
}
