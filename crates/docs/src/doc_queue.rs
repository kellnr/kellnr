use crate::compute_doc_url;
use cargo::{
    core::Workspace,
    ops::{self, CompileOptions, DocOptions},
    util::command_prelude::CompileMode,
    CargoResult, Config,
};
use common::version::Version;
use db::{Database, DbProvider, DocQueueEntry};
use flate2::read::GzDecoder;
use fs_extra::dir::{copy, CopyOptions};
use std::path::{Path, PathBuf};
use storage::kellnr_crate_storage::KellnrCrateStorage;
use tar::Archive;
use tokio::{
    fs::{create_dir_all, remove_dir_all, File},
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
) -> anyhow::Result<()> {
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
) -> anyhow::Result<()> {
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

async fn clean_up(path: &Path) -> anyhow::Result<()> {
    remove_dir_all(path).await?;
    Ok(())
}

async fn copy_dir(from: &Path, to: &Path) -> anyhow::Result<()> {
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

fn generate_docs(crate_path: impl AsRef<Path>) -> CargoResult<()> {
    let manifest_path = crate_path.as_ref().join("Cargo.toml").canonicalize()?;
    let config = Config::default()?;
    let workspace = Workspace::new(&manifest_path, &config)?;
    let compile_opts = CompileOptions::new(&config, CompileMode::Doc { deps: false })?;
    let options = DocOptions {
        open_result: false,
        compile_opts,
    };
    ops::doc(&workspace, &options)?;
    Ok(())
}
