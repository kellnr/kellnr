use std::{fs, io, path::Path, process::Command};

fn main() {
    println!("Build Kellnr - build.rs!");

    println!("cargo:rerun-if-changed=./src");
    println!("cargo:rerun-if-changed=../../ui/src");

    Command::new("npm")
        .args(["run", "build"])
        .current_dir("../../ui/src")
        .status()
        .unwrap();

    copy_dir_all("../../ui/dist", "../../static").unwrap();
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
