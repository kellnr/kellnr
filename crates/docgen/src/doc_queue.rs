use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use cargo::GlobalContext;
use cargo::core::Workspace;
use cargo::core::compiler::UserIntent;
use cargo::core::resolver::CliFeatures;
use cargo::ops::{self, CompileOptions, DocOptions, OutputFormat};
use flate2::read::GzDecoder;
use kellnr_common::background_tasks::{BackgroundTasks, FirstRound};
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::{DbProvider, DocQueueEntry};
use kellnr_docs::docs_error::DocsError;
use kellnr_docs::unpack::{ExtractLimits, unpack_crate};
use kellnr_docs::upload::upload_dir_and_prune;
use kellnr_docs::{compute_doc_url, crate_name_to_docs_name};
use kellnr_settings::{Settings, UrlPathPrefix};
use kellnr_storage::docs_storage::{DocsName, DocsStorage};
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use tokio::fs::remove_dir_all;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::build_process::DocBuilder;

pub fn doc_extraction_queue(
    db: Arc<dyn DbProvider>,
    cs: Arc<KellnrCrateStorage>,
    docs_storage: Arc<DocsStorage>,
    settings: &Settings,
    tasks: &BackgroundTasks,
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
    .with_timeout(Duration::from_secs(settings.docs.build_timeout_seconds))
    .with_shutdown(tasks.shutdown_token());
    let worker = Arc::new(DocQueueWorker {
        db,
        cs,
        docs_storage,
        path_prefix: settings.origin.path_prefix(),
        builder,
        limits: ExtractLimits::from_max_docs_size(settings.docs.max_size),
        processed: Mutex::default(),
        shutdown: tasks.shutdown_token(),
    });

    tasks.spawn_periodic(
        "docs build queue",
        Duration::from_secs(10),
        FirstRound::AfterInterval,
        move || {
            let worker = worker.clone();
            async move {
                if let Err(e) = worker.process_queue().await {
                    error!("Rustdoc generation loop failed: {e}");
                }
            }
        },
    );
}

/// Turns queued crates into hosted rustdoc, one crate at a time.
struct DocQueueWorker {
    db: Arc<dyn DbProvider>,
    cs: Arc<KellnrCrateStorage>,
    docs_storage: Arc<DocsStorage>,
    path_prefix: UrlPathPrefix,
    builder: DocBuilder,
    limits: ExtractLimits,
    /// Queue entries that were processed but could not be removed from the
    /// queue. They are only removed, but not built again.
    processed: Mutex<HashSet<i64>>,
    /// Stops the queue after the current entry and kills a running build.
    shutdown: CancellationToken,
}

/// Whether a queue entry is done or should be processed again later.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Done,
    Retry,
}

impl DocQueueWorker {
    async fn process_queue(&self) -> Result<(), DocsError> {
        let entries = self.db.get_doc_queue().await?;
        let mut built = HashSet::new();

        for entry in entries {
            if self.shutdown.is_cancelled() {
                break;
            }
            // A crate version queued several times is only built once.
            let is_first = built.insert((entry.name.clone(), entry.version.clone()));
            let outcome = if is_first && !self.is_processed(entry.id) {
                self.process_entry(&entry).await
            } else {
                Outcome::Done
            };

            // Always remove the working directory, also after failed builds.
            if let Err(e) = clean_up(&entry.path).await {
                error!("Failed to delete temporary rustdoc queue folder: {e}");
            }

            if outcome == Outcome::Done {
                self.remove_from_queue(entry.id).await;
            }
        }

        Ok(())
    }

    /// Build and publish the docs of a queue entry. Only an entry whose crate
    /// could not be read is retried, failed builds are not, as every build
    /// runs untrusted code.
    async fn process_entry(&self, entry: &DocQueueEntry) -> Outcome {
        let version = &entry.version;
        let Ok(name) = entry.canonical_name() else {
            warn!(
                "Skipping the docs build of {} {version}, the crate name is invalid",
                entry.name
            );
            return Outcome::Done;
        };
        let name = &name;
        let (original_name, contents) = match self.read_crate(name, version).await {
            Ok(Some(krate)) => krate,
            Ok(None) => {
                warn!("Skipping the docs build of {name} {version}, the crate does not exist");
                return Outcome::Done;
            }
            Err(e) => {
                error!("Failed to read {name} {version} for its docs build, retrying later: {e}");
                return Outcome::Retry;
            }
        };

        let result = self
            .build_docs(entry, name, &original_name, version, contents)
            .await;
        if matches!(result, Err(DocsError::BuildStopped)) {
            info!(
                "Stopped the docs build of {name} {version} on shutdown, it runs again after the restart"
            );
            return Outcome::Retry;
        }
        // A failure afterwards must not trigger another build.
        self.processed.lock().unwrap().insert(entry.id);
        if let Err(e) = result {
            error!("Failed to build the docs of {name} {version}: {e}");
            return Outcome::Done;
        }

        let docs_link = compute_doc_url(&original_name, version, &self.path_prefix);
        if let Err(e) = self.db.update_docs_link(name, version, &docs_link).await {
            error!("Failed to update the docs link of {name} {version}: {e}");
        }
        Outcome::Done
    }

    /// The original name and the crate file, or `None` if the crate version
    /// does not exist.
    async fn read_crate(
        &self,
        name: &CanonicalName,
        version: &Version,
    ) -> Result<Option<(OriginalName, Bytes)>, DocsError> {
        // The crate file and the folder in it are named after the original name.
        let Some(original_name) = self.db.get_original_name(name, version).await? else {
            return Ok(None);
        };
        let contents = self.cs.get(&original_name, version).await?;
        Ok(contents.map(|contents| (original_name, contents)))
    }

    async fn build_docs(
        &self,
        doc: &DocQueueEntry,
        name: &CanonicalName,
        original_name: &OriginalName,
        version: &Version,
        contents: Bytes,
    ) -> Result<(), DocsError> {
        let crate_root = format!("{original_name}-{version}");
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

        // The version may have been deleted during the build. Its docs would
        // never be deleted then.
        if self.db.get_original_name(name, version).await?.is_none() {
            return Err(DocsError::CrateDoesNotExist(
                doc.name.clone(),
                version.to_string(),
            ));
        }

        // Upload the docs directory, pruning any stale pages from a previous build.
        // The `doc` key prefix mirrors the manual-upload path (api.rs), where the
        // uploaded zip already contains a top-level `doc/` folder: both must land
        // under `{crate}/{version}/doc/...` to match `compute_doc_url`.
        let from = generated_docs_path.join("target").join("doc");
        let docs_dir = from.clone();
        let docs_name = crate_name_to_docs_name(original_name);
        tokio::task::spawn_blocking(move || remove_non_rustdoc_files(&docs_dir, &docs_name))
            .await
            .map_err(|_| DocsError::ExtractFailed)??;
        upload_dir_and_prune(
            &from,
            "doc",
            &DocsName::from(original_name),
            version,
            &self.docs_storage,
        )
        .await?;

        Ok(())
    }

    fn is_processed(&self, id: i64) -> bool {
        self.processed.lock().unwrap().contains(&id)
    }

    async fn remove_from_queue(&self, id: i64) {
        match self.db.delete_doc_queue(id).await {
            Ok(()) => {
                self.processed.lock().unwrap().remove(&id);
            }
            Err(e) => error!("Failed to remove entry {id} from the docs queue: {e}"),
        }
    }
}

/// Files and folders rustdoc writes to `target/doc` besides the folder of the
/// crate and the `src` folder. Older rustdoc versions wrote some of them.
const RUSTDOC_OUTPUT: &[&str] = &[
    "static.files",
    "search.index",
    "search.desc",
    "trait.impl",
    "type.impl",
    "implementors",
    "crates.js",
    "help.html",
    "settings.html",
    "src-files.js",
    "source-files.js",
];

fn is_rustdoc_output(name: &str, docs_name: &str) -> bool {
    name == docs_name
        || RUSTDOC_OUTPUT.contains(&name)
        || (name.starts_with("search-index") && name.to_ascii_lowercase().ends_with(".js"))
}

/// Remove everything from `doc_dir` that is not rustdoc output of the crate
/// `docs_name`.
///
/// The build runs untrusted code, which could copy files of the host into
/// `target/doc` to get them published. This limits what is published to the
/// files rustdoc writes, but the build can still write into the crate's own
/// docs. Only a confining `docs.build_wrapper` prevents that.
fn remove_non_rustdoc_files(doc_dir: &Path, docs_name: &str) -> std::io::Result<()> {
    retain_entries(doc_dir, |name| {
        name == "src" || is_rustdoc_output(name, docs_name)
    })?;
    let src = doc_dir.join("src");
    if std::fs::symlink_metadata(&src).is_ok_and(|m| m.is_dir()) {
        retain_entries(&src, |name| name == docs_name)?;
    }
    Ok(())
}

/// Remove the entries of `dir` whose name does not satisfy `keep`, without
/// following symlinks.
fn retain_entries(dir: &Path, keep: impl Fn(&str) -> bool) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if keep(&entry.file_name().to_string_lossy()) {
            continue;
        }
        debug!(
            "Not publishing {}, it is no rustdoc output",
            entry.path().display()
        );
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
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
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_storage::object_storage::MemoryStorage;
    use kellnr_storage::storage_key::StorageKey;

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
            name: name.to_string(),
            version: Version::try_from("1.0.0").unwrap(),
            path,
        }
    }

    /// Builder that runs `script` with `/bin/sh`. In the script, `$0` is
    /// `arg` and `$3` is the path of the unpacked crate.
    fn shell_builder(script: &str, arg: &Path) -> DocBuilder {
        DocBuilder::new(
            PathBuf::from("unused"),
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                script.to_string(),
                arg.display().to_string(),
            ],
            None,
        )
    }

    /// Worker with the crates `names` in a case-sensitive crate storage.
    async fn worker(
        db: MockDb,
        data_dir: &Path,
        names: &[&str],
        builder: DocBuilder,
    ) -> DocQueueWorker {
        let mut settings = kellnr_settings::test_settings();
        settings.registry.data_dir = data_dir.display().to_string();
        let cs = KellnrCrateStorage::new(&settings, Box::new(MemoryStorage::new()));
        for name in names {
            cs.put(
                &OriginalName::from_unchecked((*name).to_string()),
                &Version::try_from("1.0.0").unwrap(),
                crate_tarball(name, "1.0.0").into(),
            )
            .await
            .unwrap();
        }
        DocQueueWorker {
            db: Arc::new(db),
            cs: Arc::new(cs),
            docs_storage: Arc::new(DocsStorage::new(Box::new(MemoryStorage::new()))),
            path_prefix: UrlPathPrefix::default(),
            builder,
            limits: ExtractLimits::from_max_docs_size(1),
            processed: Mutex::default(),
            shutdown: CancellationToken::new(),
        }
    }

    /// Worker whose builds fail after appending a line to `build_log`.
    async fn failing_worker(db: MockDb, data_dir: &Path, build_log: &Path) -> DocQueueWorker {
        let builder = shell_builder("echo \"$3\" >> \"$0\"; exit 1", build_log);
        worker(db, data_dir, &["a", "b"], builder).await
    }

    fn expect_original_names(db: &mut MockDb) {
        db.expect_get_original_name()
            .returning(|name, _| Ok(Some(OriginalName::from_unchecked(name.to_string()))));
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
        expect_original_names(&mut db);
        db.expect_delete_doc_queue().times(3).returning(|_| Ok(()));
        db.expect_update_docs_link().never();
        let worker = failing_worker(db, data_dir.path(), &build_log).await;

        worker.process_queue().await.unwrap();

        let builds = std::fs::read_to_string(&build_log).unwrap();
        assert_eq!(builds.lines().count(), 2, "builds: {builds}");
        assert!(paths.iter().all(|p| !p.exists()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_stops_the_queue_and_keeps_the_stopped_build_queued() {
        let data_dir = tempfile::tempdir().unwrap();
        let build_log = data_dir.path().join("builds.log");
        let path = data_dir.path().join("queue");
        std::fs::create_dir(&path).unwrap();
        let queued = path.clone();
        let mut db = MockDb::new();
        db.expect_get_doc_queue().returning(move || {
            Ok(vec![
                queue_entry(0, "a", queued.clone()),
                queue_entry(1, "b", queued.clone()),
            ])
        });
        expect_original_names(&mut db);
        db.expect_delete_doc_queue().never();
        let shutdown = CancellationToken::new();
        let builder = shell_builder("echo \"$3\" >> \"$0\"; sleep 30", &build_log)
            .with_shutdown(shutdown.clone());
        let mut worker = worker(db, data_dir.path(), &["a", "b"], builder).await;
        worker.shutdown = shutdown.clone();

        let run = tokio::spawn(async move { worker.process_queue().await });
        while !build_log.exists() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        shutdown.cancel();
        run.await.unwrap().unwrap();

        let builds = std::fs::read_to_string(&build_log).unwrap();
        assert_eq!(builds.lines().count(), 1, "builds: {builds}");
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_queue_builds_crate_with_uppercase_letters() {
        let data_dir = tempfile::tempdir().unwrap();
        let path = data_dir.path().join("queue");
        let queued = path.clone();
        let mut db = MockDb::new();
        db.expect_get_doc_queue()
            .returning(move || Ok(vec![queue_entry(0, "my-crate", queued.clone())]));
        db.expect_get_original_name()
            .returning(|_, _| Ok(Some(OriginalName::from_unchecked("My-Crate".to_string()))));
        db.expect_update_docs_link()
            .withf(|name, _, link| {
                name.as_str() == "my_crate"
                    && link == "/docs/my-crate/1.0.0/doc/My_Crate/index.html"
            })
            .times(1)
            .returning(|_, _, _| Ok(()));
        db.expect_delete_doc_queue().times(1).returning(|_| Ok(()));
        let builder = shell_builder(
            "test -f \"$3/Cargo.toml\" && mkdir -p \"$3/target/doc/My_Crate\" && echo docs > \"$3/target/doc/My_Crate/index.html\"",
            data_dir.path(),
        );
        let worker = worker(db, data_dir.path(), &["My-Crate"], builder).await;

        worker.process_queue().await.unwrap();

        let key = DocsStorage::file_key(
            &DocsName::from(&OriginalName::try_from("my-crate").unwrap()),
            &Version::try_from("1.0.0").unwrap(),
            "doc/My_Crate/index.html",
        );
        assert!(worker.docs_storage.exists(&key).await.unwrap());
    }

    const SUCCESSFUL_BUILD: &str = "echo \"$3\" >> \"$0\"; mkdir -p \"$3/target/doc/a\" && echo docs > \"$3/target/doc/a/index.html\"";

    fn queue_dir(data_dir: &Path, i: i64) -> PathBuf {
        data_dir.join(format!("queue{i}"))
    }

    fn expect_queue(db: &mut MockDb, data_dir: &Path, names: &'static [&'static str]) {
        let data_dir = data_dir.to_path_buf();
        db.expect_get_doc_queue().returning(move || {
            Ok((0..)
                .zip(names)
                .map(|(i, name)| queue_entry(i, name, queue_dir(&data_dir, i)))
                .collect())
        });
    }

    fn build_count(build_log: &Path) -> usize {
        std::fs::read_to_string(build_log).map_or(0, |log| log.lines().count())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_queue_continues_after_database_errors() {
        let data_dir = tempfile::tempdir().unwrap();
        let build_log = data_dir.path().join("builds.log");
        let mut db = MockDb::new();
        expect_queue(&mut db, data_dir.path(), &["a", "b"]);
        expect_original_names(&mut db);
        db.expect_update_docs_link()
            .times(2)
            .returning(|_, _, _| Err(DbError::CrateNotFound("a".to_string())));
        db.expect_delete_doc_queue()
            .times(2)
            .returning(|_| Err(DbError::CrateNotFound("a".to_string())));
        let builder = shell_builder(SUCCESSFUL_BUILD, &build_log);
        let worker = worker(db, data_dir.path(), &["a", "b"], builder).await;

        worker.process_queue().await.unwrap();

        assert_eq!(build_count(&build_log), 2);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_queue_does_not_build_again_if_entry_cannot_be_removed() {
        let data_dir = tempfile::tempdir().unwrap();
        let build_log = data_dir.path().join("builds.log");
        let mut db = MockDb::new();
        expect_queue(&mut db, data_dir.path(), &["a"]);
        expect_original_names(&mut db);
        db.expect_update_docs_link().returning(|_, _, _| Ok(()));
        let mut deletes = 0;
        db.expect_delete_doc_queue().times(2).returning(move |_| {
            deletes += 1;
            if deletes == 1 {
                Err(DbError::CrateNotFound("a".to_string()))
            } else {
                Ok(())
            }
        });
        let builder = shell_builder(SUCCESSFUL_BUILD, &build_log);
        let worker = worker(db, data_dir.path(), &["a"], builder).await;

        worker.process_queue().await.unwrap();
        worker.process_queue().await.unwrap();

        assert_eq!(build_count(&build_log), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_queue_retries_entry_if_crate_cannot_be_read() {
        let data_dir = tempfile::tempdir().unwrap();
        let build_log = data_dir.path().join("builds.log");
        let mut db = MockDb::new();
        expect_queue(&mut db, data_dir.path(), &["a"]);
        db.expect_get_original_name()
            .returning(|_, _| Err(DbError::CrateNotFound("a".to_string())));
        db.expect_delete_doc_queue().never();
        let builder = shell_builder(SUCCESSFUL_BUILD, &build_log);
        let worker = worker(db, data_dir.path(), &["a"], builder).await;

        worker.process_queue().await.unwrap();

        assert_eq!(build_count(&build_log), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_queue_does_not_upload_docs_of_version_deleted_during_build() {
        let data_dir = tempfile::tempdir().unwrap();
        let build_log = data_dir.path().join("builds.log");
        let mut db = MockDb::new();
        expect_queue(&mut db, data_dir.path(), &["a"]);
        let mut lookups = 0;
        db.expect_get_original_name().returning(move |name, _| {
            lookups += 1;
            Ok((lookups == 1).then(|| OriginalName::from_unchecked(name.to_string())))
        });
        db.expect_update_docs_link().never();
        db.expect_delete_doc_queue().times(1).returning(|_| Ok(()));
        let builder = shell_builder(SUCCESSFUL_BUILD, &build_log);
        let worker = worker(db, data_dir.path(), &["a"], builder).await;

        worker.process_queue().await.unwrap();

        assert_eq!(build_count(&build_log), 1);
        let prefix = DocsStorage::version_prefix(
            &DocsName::from(&OriginalName::try_from("a").unwrap()),
            &Version::try_from("1.0.0").unwrap(),
        );
        assert!(worker.docs_storage.list(&prefix).await.unwrap().is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_queue_publishes_only_rustdoc_output() {
        let data_dir = tempfile::tempdir().unwrap();
        let mut db = MockDb::new();
        expect_queue(&mut db, data_dir.path(), &["a"]);
        expect_original_names(&mut db);
        db.expect_update_docs_link().returning(|_, _, _| Ok(()));
        db.expect_delete_doc_queue().returning(|_| Ok(()));
        let builder = shell_builder(
            "mkdir -p \"$3/target/doc\" && cd \"$3/target/doc\" && mkdir -p a src/a src/other && touch a/index.html src/a/lib.rs.html src/other/x.html crates.js leak.txt",
            data_dir.path(),
        );
        let worker = worker(db, data_dir.path(), &["a"], builder).await;

        worker.process_queue().await.unwrap();

        let prefix = DocsStorage::version_prefix(
            &DocsName::from(&OriginalName::try_from("a").unwrap()),
            &Version::try_from("1.0.0").unwrap(),
        );
        let mut keys: Vec<String> = worker
            .docs_storage
            .list(&prefix)
            .await
            .unwrap()
            .into_iter()
            .map(StorageKey::into_inner)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "a/1.0.0/doc/a/index.html",
                "a/1.0.0/doc/crates.js",
                "a/1.0.0/doc/src/a/lib.rs.html",
            ]
        );
    }

    #[test]
    fn remove_non_rustdoc_files_keeps_rustdoc_output_of_the_crate() {
        let dir = tempfile::tempdir().unwrap();
        let kept = [
            "My_Crate/index.html",
            "static.files/rustdoc.css",
            "search.index/root.js",
            "trait.impl/core/clone/trait.Clone.js",
            "type.impl/x.js",
            "implementors/x.js",
            "src/My_Crate/lib.rs.html",
            "crates.js",
            "help.html",
            "settings.html",
            "src-files.js",
            "search-index1.70.0.js",
        ];
        let removed = [
            "src/other/secret.html",
            "debug/secret",
            ".lock",
            "secret.txt",
        ];
        for file in kept.iter().chain(&removed) {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }

        remove_non_rustdoc_files(dir.path(), "My_Crate").unwrap();

        for file in kept {
            assert!(dir.path().join(file).exists(), "{file}");
        }
        for file in removed {
            assert!(!dir.path().join(file).exists(), "{file}");
        }
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
