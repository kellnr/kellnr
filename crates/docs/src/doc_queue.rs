use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use cargo::GlobalContext;
use cargo::core::Workspace;
use cargo::core::compiler::UserIntent;
use cargo::core::resolver::CliFeatures;
use cargo::ops::{self, CompileOptions, DocOptions, OutputFormat};
use flate2::read::GzDecoder;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::{DbProvider, DocQueueEntry};
use kellnr_settings::Settings;
use kellnr_storage::docs_storage::DocsStorage;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use tokio::fs::remove_dir_all;
use tracing::error;

use crate::build_process::DocBuilder;
use crate::compute_doc_url;
use crate::docs_error::DocsError;
use crate::unpack::{ExtractLimits, unpack_crate};
use crate::upload::upload_dir_and_prune;

pub fn doc_extraction_queue(
    db: Arc<dyn DbProvider>,
    cs: Arc<KellnrCrateStorage>,
    docs_storage: Arc<DocsStorage>,
    settings: &Settings,
) {
    let program = match std::env::current_exe() {
        Ok(program) => program,
        Err(e) => {
            error!("Rustdoc generation disabled, cannot locate the kellnr binary: {e}");
            return;
        }
    };
    let builder = DocBuilder::new(
        program,
        settings.docs.build_wrapper.clone(),
        settings
            .proxy
            .cratesio_index_override()
            .map(ToString::to_string),
    )
    .with_timeout(Duration::from_secs(settings.docs.build_timeout_seconds));
    let worker = DocQueueWorker {
        db,
        cs,
        docs_storage,
        path_prefix: settings.origin.path.clone(),
        builder,
        limits: ExtractLimits::from_max_docs_size(settings.docs.max_size),
    };

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            if let Err(e) = worker.process_queue().await {
                error!("Rustdoc generation loop failed: {e}");
            }
        }
    });
}

/// Turns queued crates into hosted rustdoc, one crate at a time.
struct DocQueueWorker {
    db: Arc<dyn DbProvider>,
    cs: Arc<KellnrCrateStorage>,
    docs_storage: Arc<DocsStorage>,
    path_prefix: String,
    builder: DocBuilder,
    limits: ExtractLimits,
}

impl DocQueueWorker {
    async fn process_queue(&self) -> Result<(), DocsError> {
        let entries = self.db.get_doc_queue().await?;
        let mut built = HashSet::new();

        for entry in entries {
            // A crate version queued several times is only built once.
            let is_first = built.insert((entry.normalized_name.to_string(), entry.version.clone()));
            let result = if is_first {
                Some(self.extract_docs(&entry).await)
            } else {
                None
            };

            // Always remove the working directory, also after failed builds.
            if let Err(e) = clean_up(&entry.path).await {
                error!("Failed to delete temporary rustdoc queue folder: {e}");
            }

            match result {
                Some(Ok(())) => {
                    let version = Version::from_unchecked_str(&entry.version);
                    let docs_link =
                        compute_doc_url(&entry.normalized_name, &version, &self.path_prefix);
                    self.db
                        .update_docs_link(&entry.normalized_name, &version, &docs_link)
                        .await?;
                }
                Some(Err(e)) => error!("Failed to extract docs from crate: {e}"),
                None => {}
            }
            self.db.delete_doc_queue(entry.id).await?;
        }

        Ok(())
    }

    async fn extract_docs(&self, doc: &DocQueueEntry) -> Result<(), DocsError> {
        // TODO: Only works if normalized name = original name -> Need to get original name from db
        let orig_name = OriginalName::from_unchecked(doc.normalized_name.to_string());
        let version = Version::from_unchecked_str(&doc.version);
        let contents = self.cs.get(&orig_name, &version).await?.ok_or_else(|| {
            error!("Failed to get crate from storage");
            DocsError::CrateDoesNotExist(doc.normalized_name.to_string(), doc.version.clone())
        })?;
        let crate_root = format!("{}-{}", doc.normalized_name, doc.version);
        let tar = GzDecoder::new(std::io::Cursor::new(contents));
        let dest = doc.path.clone();
        let root = crate_root.clone();
        let limits = self.limits;
        tokio::task::spawn_blocking(move || unpack_crate(tar, &dest, &root, limits))
            .await
            .map_err(|_| DocsError::ExtractFailed)??;

        // Generate the docs
        let generated_docs_path = &doc.path.join(crate_root);
        strip_rust_toolchain_files(generated_docs_path).await?;

        // The build runs untrusted code (build scripts, proc macros), so it runs
        // in a separate process without the Kellnr environment. See `DocBuilder`.
        self.builder.build(generated_docs_path).await?;

        // Upload the docs directory, pruning any stale pages from a previous build.
        // The `doc` key prefix mirrors the manual-upload path (api.rs), where the
        // uploaded zip already contains a top-level `doc/` folder: both must land
        // under `{crate}/{version}/doc/...` to match `compute_doc_url`.
        let from = generated_docs_path.join("target").join("doc");
        upload_dir_and_prune(
            &from,
            "doc",
            &doc.normalized_name.to_string(),
            &doc.version,
            &self.docs_storage,
        )
        .await?;

        Ok(())
    }
}

async fn clean_up(path: &Path) -> Result<(), DocsError> {
    match remove_dir_all(path).await {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
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

/// Build the cargo [`GlobalContext`] used to document a crate.
///
/// When a custom crates.io proxy index is configured (`cratesio_index`), point
/// cargo's `crates-io` source at it via source replacement so dependency
/// resolution for `cargo doc` honors `proxy.index` instead of fetching from the
/// upstream `index.crates.io`. See issue #1185.
fn build_doc_context(cratesio_index: Option<&str>) -> Result<GlobalContext, DocsError> {
    let mut ctx = GlobalContext::default().map_err(|e| DocsError::CargoError(e.to_string()))?;

    if let Some(index) = cratesio_index {
        let cli_config = vec![
            "source.crates-io.replace-with=\"kellnr-proxy\"".to_string(),
            format!("source.kellnr-proxy.registry=\"sparse+{index}\""),
        ];
        ctx.configure(0, false, None, false, false, false, &None, &[], &cli_config)
            .map_err(|e| DocsError::CargoError(e.to_string()))?;
    }

    Ok(ctx)
}

/// Select the features to document the crate with.
///
/// Honors the feature keys of the docs.rs metadata table
/// (`[package.metadata.docs.rs]`): `all-features`, `features` and
/// `no-default-features`. Unlike docs.rs, `all-features` defaults to `true`,
/// which keeps the behavior Kellnr always had for crates without the table.
/// Crates whose features are mutually exclusive set `all-features = false`
/// to opt out. See issue #1407.
fn doc_features(workspace: &Workspace) -> Result<CliFeatures, DocsError> {
    let package = workspace
        .current()
        .map_err(|e| DocsError::CargoError(e.to_string()))?;
    // `[package.metadata.docs.rs]` is a dotted key, so `rs` is nested in `docs`.
    let docs_rs = package
        .manifest()
        .custom_metadata()
        .and_then(|m| m.get("docs"))
        .and_then(|m| m.get("rs"));

    let flag = |key: &str| docs_rs.and_then(|m| m.get(key)?.as_bool());
    let all_features = flag("all-features").unwrap_or(true);
    let no_default_features = flag("no-default-features").unwrap_or(false);
    let features: Vec<String> = docs_rs
        .and_then(|m| m.get("features"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|f| f.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    CliFeatures::from_command_line(&features, all_features, !no_default_features)
        .map_err(|e| DocsError::CargoError(e.to_string()))
}

/// Document the crate at `crate_path` with cargo in the current process.
///
/// This runs the crate's build scripts and proc macros. It is only called by
/// the hidden `doc-build` subcommand, which [`DocBuilder`] runs as a separate
/// process with a cleared environment.
pub fn generate_docs(
    crate_path: impl AsRef<Path>,
    cratesio_index: Option<&str>,
) -> Result<(), DocsError> {
    let manifest_path = crate_path.as_ref().join("Cargo.toml").canonicalize()?;
    let ctx = build_doc_context(cratesio_index)?;
    let workspace =
        Workspace::new(&manifest_path, &ctx).map_err(|e| DocsError::CargoError(e.to_string()))?;
    let compile_opts = CompileOptions {
        cli_features: doc_features(&workspace)?,
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
    use std::path::PathBuf;

    use cargo::core::SourceId;
    use cargo::sources::SourceConfigMap;
    use kellnr_common::normalized_name::NormalizedName;
    use kellnr_db::mock::MockDb;
    use kellnr_storage::fs_storage::FSStorage;

    use super::*;

    fn crate_tarball(name: &str, version: &str) -> Vec<u8> {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        let manifest = format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\n");
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(
                &mut header,
                format!("{name}-{version}/Cargo.toml"),
                manifest.as_bytes(),
            )
            .unwrap();
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn queue_entry(id: i64, name: &str, path: PathBuf) -> DocQueueEntry {
        DocQueueEntry {
            id,
            normalized_name: NormalizedName::from_unchecked(name.to_string()),
            version: "1.0.0".to_string(),
            path,
        }
    }

    /// Worker whose builds fail after appending a line to `build_log`.
    async fn failing_worker(db: MockDb, data_dir: &Path, build_log: &Path) -> DocQueueWorker {
        let mut settings = kellnr_settings::test_settings();
        settings.registry.data_dir = data_dir.display().to_string();
        let storage = FSStorage::new(data_dir.join("crates").to_str().unwrap()).unwrap();
        let cs = KellnrCrateStorage::new(&settings, Box::new(storage));
        for name in ["a", "b"] {
            cs.put(
                &OriginalName::from_unchecked(name.to_string()),
                &Version::from_unchecked_str("1.0.0"),
                crate_tarball(name, "1.0.0").into(),
            )
            .await
            .unwrap();
        }
        let docs_storage = FSStorage::new(data_dir.join("docs").to_str().unwrap()).unwrap();
        DocQueueWorker {
            db: Arc::new(db),
            cs: Arc::new(cs),
            docs_storage: Arc::new(DocsStorage::new(Box::new(docs_storage))),
            path_prefix: String::new(),
            builder: DocBuilder::new(
                PathBuf::from("unused"),
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "echo \"$3\" >> \"$0\"; exit 1".to_string(),
                    build_log.display().to_string(),
                ],
                None,
            ),
            limits: ExtractLimits::from_max_docs_size(1),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_queue_builds_duplicates_once_and_always_cleans_up() {
        let data_dir = tempfile::tempdir().unwrap();
        let build_log = data_dir.path().join("builds.log");
        let paths: Vec<PathBuf> = (0..3)
            .map(|i| {
                let path = data_dir.path().join(format!("queue{i}"));
                std::fs::create_dir(&path).unwrap();
                path
            })
            .collect();
        let queued = paths.clone();
        let mut db = MockDb::new();
        db.expect_get_doc_queue().returning(move || {
            Ok(vec![
                queue_entry(0, "a", queued[0].clone()),
                queue_entry(1, "a", queued[1].clone()),
                queue_entry(2, "b", queued[2].clone()),
            ])
        });
        db.expect_delete_doc_queue().times(3).returning(|_| Ok(()));
        db.expect_update_docs_link().never();
        let worker = failing_worker(db, data_dir.path(), &build_log).await;

        worker.process_queue().await.unwrap();

        let builds = std::fs::read_to_string(&build_log).unwrap();
        assert_eq!(builds.lines().count(), 2, "builds: {builds}");
        assert!(paths.iter().all(|p| !p.exists()));
    }

    #[tokio::test]
    async fn clean_up_ignores_missing_directory() {
        let dir = tempfile::tempdir().unwrap();

        clean_up(&dir.path().join("missing")).await.unwrap();
    }

    #[test]
    fn no_index_override_does_not_inject_proxy_source() {
        let ctx = build_doc_context(None).unwrap();
        // Without an override we never define the kellnr-proxy source, so cargo
        // keeps whatever crates.io source the ambient environment provides
        // (the upstream index.crates.io in the kellnr container). Asserting on
        // `crates-io.replace-with` directly would be environment-dependent,
        // since a developer's ~/.cargo/config.toml may itself replace it.
        assert!(
            ctx.get_string("source.kellnr-proxy.registry")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn index_override_replaces_crates_io_source_with_proxy() {
        let ctx = build_doc_context(Some("https://rsproxy.cn/index/")).unwrap();

        // The CLI config overrides are well-formed and applied by cargo.
        assert_eq!(
            ctx.get_string("source.crates-io.replace-with")
                .unwrap()
                .map(|v| v.val),
            Some("kellnr-proxy".to_string())
        );

        // Cargo's own source resolution maps the crates.io source to the
        // configured proxy index instead of index.crates.io. See issue #1185.
        let map = SourceConfigMap::new(&ctx).unwrap();
        let crates_io = SourceId::crates_io(&ctx).unwrap();
        let source = map.load(crates_io, &HashSet::new()).unwrap();
        let replaced = source.replaced_source_id();
        assert!(!replaced.is_crates_io());
        assert!(
            replaced.url().as_str().contains("rsproxy.cn/index/"),
            "expected replacement source to point at the proxy index, got {}",
            replaced.url()
        );
    }

    fn features_for(manifest: &str) -> CliFeatures {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src").join("lib.rs"), "").unwrap();
        let ctx = build_doc_context(None).unwrap();
        let manifest_path = dir.path().join("Cargo.toml").canonicalize().unwrap();
        let workspace = Workspace::new(&manifest_path, &ctx).unwrap();
        doc_features(&workspace).unwrap()
    }

    const PACKAGE: &str = "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\na = []\nb = []\n";

    fn feature_names(features: &CliFeatures) -> Vec<String> {
        let mut names: Vec<String> = features.features.iter().map(ToString::to_string).collect();
        names.sort();
        names
    }

    #[test]
    fn doc_features_default_to_all_features_without_metadata() {
        let features = features_for(PACKAGE);

        assert!(features.all_features);
        assert!(features.uses_default_features);
        assert!(features.features.is_empty());
    }

    #[test]
    fn doc_features_honor_docs_rs_metadata() {
        let features = features_for(&format!(
            "{PACKAGE}\n[package.metadata.docs.rs]\nall-features = false\nno-default-features = true\nfeatures = [\"a\", \"b\"]\n"
        ));

        assert!(!features.all_features);
        assert!(!features.uses_default_features);
        assert_eq!(feature_names(&features), vec!["a", "b"]);
    }

    #[test]
    fn doc_features_keep_all_features_when_only_features_are_listed() {
        let features = features_for(&format!(
            "{PACKAGE}\n[package.metadata.docs.rs]\nfeatures = [\"a\"]\n"
        ));

        assert!(features.all_features);
        assert!(features.uses_default_features);
        assert_eq!(feature_names(&features), vec!["a"]);
    }

    #[test]
    fn doc_features_ignore_unrelated_metadata() {
        let features = features_for(&format!(
            "{PACKAGE}\n[package.metadata.other]\nall-features = false\n"
        ));

        assert!(features.all_features);
    }

    #[test]
    fn generate_docs_builds_crate_with_mutually_exclusive_features() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            format!("{PACKAGE}\n[package.metadata.docs.rs]\nall-features = false\n"),
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src").join("lib.rs"),
            "#[cfg(all(feature = \"a\", feature = \"b\"))]\ncompile_error!(\"a and b are exclusive\");\n",
        )
        .unwrap();

        generate_docs(dir.path(), None).unwrap();

        assert!(dir.path().join("target/doc/x/index.html").exists());
    }

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
