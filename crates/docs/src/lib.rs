pub mod api;
mod doc_archive;
pub mod doc_queue;
pub mod doc_queue_response;
pub mod upload_response;

use common::version::Version;
use settings::Settings;
use std::convert::TryFrom;
use std::path::Path;

pub fn get_latest_doc_url(crate_name: &str, settings: &Settings) -> Option<String> {
    let version = get_latest_version_with_doc(crate_name, settings);
    version.map(|v| compute_doc_url(crate_name, &v))
}

pub fn get_doc_url(crate_name: &str, crate_version: &Version, docs_path: &Path) -> Option<String> {
    let docs_name = crate_name_to_docs_name(crate_name);

    if doc_exists(crate_name, crate_version, docs_path) {
        Some(format!(
            "/docs/{}/{}/doc/{}/index.html",
            crate_name, crate_version, docs_name
        ))
    } else {
        None
    }
}

pub fn compute_doc_url(crate_name: &str, crate_version: &Version) -> String {
    let docs_name = crate_name_to_docs_name(crate_name);
    format!(
        "/docs/{}/{}/doc/{}/index.html",
        crate_name, crate_version, docs_name
    )
}

fn crate_name_to_docs_name(crate_name: &str) -> String {
    // Cargo replaces the `-` with `_` in the crate name when
    // docs are generated. As such, the docs folder name is not "foo-bar" but "foo_bar".
    crate_name.replace('-', "_")
}

fn doc_exists(crate_name: &str, crate_version: &str, docs_path: &Path) -> bool {
    let docs_name = crate_name_to_docs_name(crate_name);
    docs_path
        .join(crate_name)
        .join(crate_version)
        .join("doc")
        .join(docs_name)
        .join("index.html")
        .exists()
}

fn get_latest_version_with_doc(crate_name: &str, settings: &Settings) -> Option<Version> {
    let versions_path = settings.docs_path().join(crate_name);
    let version_folders = match std::fs::read_dir(versions_path) {
        Err(_) => return None,
        Ok(f) => f,
    };

    let mut versions: Vec<Version> = version_folders
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .flat_map(|dir| Version::try_from(&dir.file_name().to_string_lossy().to_string()))
        .collect();

    // Sort and reverse the order such that the biggest version
    // for which docs exist will be returned.
    versions.sort();
    versions.reverse();
    versions
        .into_iter()
        .find(|v| doc_exists(crate_name, &v.to_string(), &settings.docs_path()))
}

pub async fn delete(
    crate_name: &str,
    crate_version: &str,
    settings: &Settings,
) -> Result<(), std::io::Error> {
    // Delete the docs folder for the crate version.
    let docs_path = settings.docs_path().join(crate_name).join(crate_version);
    if docs_path.exists() {
        tokio::fs::remove_dir_all(docs_path).await?;
    }

    // If it was the last version, delete the empty crate docs folder.
    if get_latest_version_with_doc(crate_name, settings).is_none() {
        let crate_path = settings.docs_path().join(crate_name);
        if crate_path.exists() {
            tokio::fs::remove_dir_all(crate_path).await?;
        }
    }

    Ok(())
}
