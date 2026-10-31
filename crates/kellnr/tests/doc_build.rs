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
