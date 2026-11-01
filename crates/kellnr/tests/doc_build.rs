use std::path::{Path, PathBuf};

use kellnr_docgen::build_process::DocBuilder;

fn write_crate(dir: &Path) {
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"doc-build-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/lib.rs"), "/// Documented.\npub fn f() {}\n").unwrap();
}

#[tokio::test]
async fn doc_builder_documents_crate_in_kellnr_child_process() {
    let dir = tempfile::tempdir().unwrap();
    write_crate(dir.path());
    let builder = DocBuilder::new(PathBuf::from(env!("CARGO_BIN_EXE_kellnr")), vec![], None);

    builder.build(dir.path()).await.unwrap();

    assert!(
        dir.path()
            .join("target/doc/doc_build_test/index.html")
            .exists()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn doc_builder_documents_crate_into_crate_folder_when_target_dir_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let crate_dir = dir.path().join("crate");
    std::fs::create_dir(&crate_dir).unwrap();
    write_crate(&crate_dir);
    let other_target = dir.path().join("other-target");
    // Run the build with a target dir set in its environment, as Kellnr
    // passes on the `CARGO_*` variables it was started with.
    let wrapper = vec![
        "/usr/bin/env".to_string(),
        format!("CARGO_TARGET_DIR={}", other_target.display()),
        format!("CARGO_BUILD_TARGET_DIR={}", other_target.display()),
    ];
    let builder = DocBuilder::new(PathBuf::from(env!("CARGO_BIN_EXE_kellnr")), wrapper, None);

    builder.build(&crate_dir).await.unwrap();

    assert!(
        crate_dir
            .join("target/doc/doc_build_test/index.html")
            .exists()
    );
    assert!(!other_target.exists());
}
