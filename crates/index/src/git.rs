use anyhow::{Context, Result};
use rocket::tokio::{
    fs::{remove_file, OpenOptions},
    io::AsyncWriteExt,
    sync::{mpsc::Receiver, Mutex},
};
use std::process::Command;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub enum ThreadSyncMsg {
    StopLoop,
}
pub type LoopSync = Arc<Mutex<Receiver<ThreadSyncMsg>>>;

pub fn repo_exists(repo_path: &Path) -> bool {
    Path::exists(&repo_path.join(".git").join("index"))
}

pub fn is_locked(repo_path: &Path) -> bool {
    Path::exists(&lock_file(repo_path))
}

pub async fn unlock(repo_path: &Path) -> Result<()> {
    remove_file(&lock_file(repo_path))
        .await
        .with_context(|| "Failed to unlock repository.")
}

pub fn clone_repo(repo_path: &Path, repo_url: &str) -> Result<()> {
    let _ = Command::new("git")
        .current_dir(repo_path)
        .arg("clone")
        .arg(repo_url)
        .arg(".")
        .output()
        .with_context(|| "Failed to clone repository")?;

    Ok(())
}

pub async fn add_file_and_commit(
    file_path: &Path,
    file_content: &str,
    repo_path: &Path,
    commit_msg: &str,
) -> Result<()> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true) // Delete file content if already exists
        .open(file_path)
        .await?
        .write_all(file_content.as_bytes())
        .await?;

    add_and_commit(repo_path, commit_msg).await?;
    Ok(())
}

pub fn add_all(repo_path: &Path) -> Result<()> {
    let _ = Command::new("git")
        .current_dir(repo_path)
        .arg("add")
        .arg(".")
        .output()
        .with_context(|| "Unable to add to git")?;

    Ok(())
}

pub async fn add_and_commit(repo_path: &Path, commit_msg: &str) -> Result<()> {
    let repo_path2 = PathBuf::from(repo_path);
    let commit_msg2 = commit_msg.to_string();

    rocket::tokio::task::spawn_blocking(move || {
        add_all(&repo_path2)?;
        commit(&commit_msg2, &repo_path2).with_context(|| "Unable to commit to index")?;
        Ok(())
    })
    .await?
}

fn lock_file(repo_path: &Path) -> PathBuf {
    repo_path.join(".git").join("index.lock")
}

pub fn commit(msg: &str, repo_path: &Path) -> Result<()> {
    configure_repo(repo_path)?;
    let _ = Command::new("git")
        .current_dir(repo_path)
        .arg("commit")
        .arg("-m")
        .arg(msg)
        .arg("--no-ahead-behind")
        .output()
        .with_context(|| "Unable to commit to index")?;

    Ok(())
}

pub fn init(repo_path: &Path) -> Result<()> {
    let _ = Command::new("git")
        .current_dir(repo_path)
        .arg("init")
        .output()
        .with_context(|| "Failed to initialize repository")?;

    Ok(())
}

pub fn configure_repo(repo_path: &Path) -> Result<()> {
    let _ = Command::new("git")
        .current_dir(repo_path)
        .arg("config")
        .arg("pull.rebase")
        .arg("false")
        .output()
        .with_context(|| "Failed to configure merge strategy")?;

    let _ = Command::new("git")
        .current_dir(repo_path)
        .arg("config")
        .arg("user.email")
        .arg("kellnr@kellnr.io")
        .output()
        .with_context(|| "Failed to configure user email")?;

    let _ = Command::new("git")
        .current_dir(repo_path)
        .arg("config")
        .arg("user.name")
        .arg("kellnr")
        .output()
        .with_context(|| "Failed to configure user name")?;

    Ok(())
}

pub fn pull(repo_path: &Path) -> Result<()> {
    let _ = Command::new("git")
        .current_dir(repo_path)
        .arg("pull")
        .arg("--no-edit")
        .output()
        .with_context(|| "Failed to pull and merge remote branch")?;

    Ok(())
}
