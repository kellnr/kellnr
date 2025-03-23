use crate::{compute_doc_url, docs_error::DocsError};
use cargo::{
    GlobalContext,
    core::{Workspace, resolver::CliFeatures},
    ops::{self, CompileOptions, DocOptions, OutputFormat},
    util::command_prelude::CompileMode,
};
use common::version::Version;
use db::{Database, DbProvider, DocQueueEntry};
use flate2::read::GzDecoder;
use fs_extra::dir::{CopyOptions, copy};
use std::path::{Path, PathBuf};
use storage::kellnr_crate_storage::KellnrCrateStorage;
use tar::Archive;
use tokio::{
    fs::{File, create_dir_all, remove_dir_all},
    io::AsyncReadExt,
};
use tracing::error;

pub async fn doc_extraction_queue(db: Database, cs: KellnrCrateStorage, docs_path: PathBuf) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            if let Err(e) = inner_loop(&db, &cs, &docs_path).await {
                error!("Rustdoc generation loop failed: {e}");
            }
        }
    });
}

async fn inner_loop(
    db: &impl DbProvider,
    cs: &KellnrCrateStorage,
    docs_path: &Path,
) -> Result<(), DocsError> {
    let entries = db.get_doc_queue().await?;

    for entry in entries {
        if let Err(e) = extract_docs(&entry, cs, docs_path).await {
            error!("Failed to extract docs from crate: {e}");
        } else {
            if let Err(e) = clean_up(&entry.path).await {
                error!("Failed to delete temporary rustdoc queue folder: {e}")
            }

            let version = Version::from_unchecked_str(&entry.version);
            let docs_link = compute_doc_url(&entry.krate, &version);
            db.update_docs_link(&entry.krate, &version, &docs_link)
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
    let crate_path = cs.crate_path(&doc.krate, &doc.version);

    // Unpack crate
    let mut tar_gz = File::open(&crate_path).await?;
    let mut contents = vec![];
    tar_gz.read_to_end(&mut contents).await?;
    let tar = GzDecoder::new(std::io::Cursor::new(contents));
    let mut archive = Archive::new(tar);
    archive.unpack(&doc.path)?;

    // Generate the docs
    let generated_docs_path = &doc.path.join(format!("{}-{}", doc.krate, doc.version));
    generate_docs(generated_docs_path)?;

    // Copy the docs directory
    let from = generated_docs_path.join("target").join("doc");
    let to = docs_path.join(doc.krate.to_string()).join(&doc.version);
    copy_dir(&from, &to).await?;

    Ok(())
}

async fn clean_up(path: &Path) -> Result<(), DocsError> {
    remove_dir_all(path).await?;
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
            CompileMode::Doc {
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
