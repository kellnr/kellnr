pub mod api;
mod doc_archive;
pub mod doc_queue;
pub mod doc_queue_response;
pub mod docs_error;
pub mod upload_response;

use std::convert::TryFrom;
use std::path::Path;

use kellnr_common::version::Version;
use kellnr_settings::Settings;

pub fn get_latest_doc_url(crate_name: &str, settings: &Settings) -> Option<String> {
    let version = get_latest_version_with_doc(crate_name, settings);
    version.map(|v| compute_doc_url(crate_name, &v, &settings.origin.path))
}

pub fn get_doc_url(
    crate_name: &str,
    crate_version: &Version,
    docs_path: &Path,
    path_prefix: &str,
) -> Option<String> {
    let docs_name = crate_name_to_docs_name(crate_name);
    let path_prefix = path_prefix.trim();

    if doc_exists(crate_name, crate_version, docs_path) {
        Some(format!(
            "{path_prefix}/docs/{crate_name}/{crate_version}/doc/{docs_name}/index.html"
        ))
    } else {
        None
    }
}

pub fn compute_doc_url(crate_name: &str, crate_version: &Version, path_prefix: &str) -> String {
    let docs_name = crate_name_to_docs_name(crate_name);
    let path_prefix = path_prefix.trim();
    format!("{path_prefix}/docs/{crate_name}/{crate_version}/doc/{docs_name}/index.html")
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
    let Ok(version_folders) = std::fs::read_dir(versions_path) else {
        return None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_doc_url_without_path_prefix() {
        let version = Version::try_from("1.0.0").unwrap();
        let url = compute_doc_url("my-crate", &version, "");
        assert_eq!(url, "/docs/my-crate/1.0.0/doc/my_crate/index.html");
    }

    #[test]
    fn compute_doc_url_with_path_prefix() {
        let version = Version::try_from("1.0.0").unwrap();
        let url = compute_doc_url("my-crate", &version, "/kellnr");
        assert_eq!(url, "/kellnr/docs/my-crate/1.0.0/doc/my_crate/index.html");
    }

    #[test]
    fn compute_doc_url_trims_whitespace_from_path_prefix() {
        let version = Version::try_from("1.0.0").unwrap();
        let url = compute_doc_url("my-crate", &version, "  /kellnr  ");
        assert_eq!(url, "/kellnr/docs/my-crate/1.0.0/doc/my_crate/index.html");
    }

    #[test]
    fn compute_doc_url_replaces_hyphen_with_underscore_in_docs_name() {
        let version = Version::try_from("2.0.0-beta1").unwrap();
        let url = compute_doc_url("foo-bar-baz", &version, "");
        assert_eq!(url, "/docs/foo-bar-baz/2.0.0-beta1/doc/foo_bar_baz/index.html");
    }
}
