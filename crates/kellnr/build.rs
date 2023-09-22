use std::{fs, io, path::Path, process::Command};

static UI_DIR: &str = "../../ui";
static UI_DIR_SRC: &str = "../../ui/src";
static UI_DIST_DIR: &str = "../../ui/dist";
static STATIC_DIR: &str = "../../static";

fn main() {
    println!("Build Kellnr - build.rs!");

    println!("cargo:rerun-if-changed={}", UI_DIR_SRC);

    install_ui_deps();
    build_ui();
    copy_dir_all(UI_DIST_DIR, STATIC_DIR).unwrap();
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn install_ui_deps() {
    if !Path::new("./ui/node_modules").exists() {
        println!("Installing node dependencies...");
        Command::new("npm")
            .args(["install"])
            .current_dir(UI_DIR)
            .status()
            .unwrap();
    }
}

fn build_ui() {
    println!("Building UI...");
    Command::new("npm")
        .args(["run", "build"])
        .current_dir(UI_DIR)
        .status()
        .unwrap();
}
