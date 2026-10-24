use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use kellnr_settings::cli::{DOC_BUILD_CRATESIO_INDEX_ARG, DOC_BUILD_SUBCOMMAND};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use crate::docs_error::DocsError;

/// Environment variables passed on to the docs build process. Everything else,
/// especially the `KELLNR_*` settings with their secrets, is removed.
const ALLOWED_ENV_VARS: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LANG",
    "LC_ALL",
    "TMPDIR",
    "TEMP",
    "TMP",
    "CARGO_HOME",
    "CARGO_HTTP_CAINFO",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    // Required to start processes on Windows.
    "SYSTEMROOT",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "PATHEXT",
    "COMSPEC",
];

/// Keep at most this many bytes of the build output for error messages.
const OUTPUT_TAIL_BYTES: usize = 16 * 1024;

/// Runs docs builds in a separate process with a minimal environment.
///
/// The build compiles untrusted crates, which run build scripts and proc
/// macros. The child process re-executes the Kellnr binary with the hidden
/// [`DOC_BUILD_SUBCOMMAND`], optionally prefixed by an operator supplied
/// wrapper (e.g. bubblewrap) for further confinement.
#[derive(Debug, Clone)]
pub struct DocBuilder {
    program: PathBuf,
    wrapper: Vec<String>,
    cratesio_index: Option<String>,
}

impl DocBuilder {
    pub fn new(program: PathBuf, wrapper: Vec<String>, cratesio_index: Option<String>) -> Self {
        Self {
            program,
            wrapper,
            cratesio_index,
        }
    }

    /// Build the docs of the crate at `crate_path` into `crate_path/target/doc`.
    pub async fn build(&self, crate_path: &Path) -> Result<(), DocsError> {
        let mut child = self.command(crate_path).spawn().map_err(|e| {
            DocsError::CargoError(format!("failed to start docs build process: {e}"))
        })?;
        let output = tokio::spawn(read_tail(
            child.stderr.take().expect("stderr is piped"),
            OUTPUT_TAIL_BYTES,
        ));

        let status = child.wait().await?;
        let output = output.await.unwrap_or_default();
        if status.success() {
            Ok(())
        } else {
            Err(DocsError::CargoError(format!(
                "docs build failed ({status}): {output}"
            )))
        }
    }

    fn command(&self, crate_path: &Path) -> Command {
        let mut args: Vec<OsString> = self.wrapper.iter().map(OsString::from).collect();
        args.push(self.program.clone().into_os_string());
        args.push(DOC_BUILD_SUBCOMMAND.into());
        args.push(crate_path.into());
        if let Some(index) = &self.cratesio_index {
            args.push(format!("--{DOC_BUILD_CRATESIO_INDEX_ARG}").into());
            args.push(index.into());
        }

        let mut command = Command::new(&args[0]);
        command
            .args(&args[1..])
            .env_clear()
            .envs(allowed_env(std::env::vars_os()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command
    }
}

fn allowed_env(
    vars: impl Iterator<Item = (OsString, OsString)>,
) -> impl Iterator<Item = (OsString, OsString)> {
    vars.filter(|(key, _)| key.to_str().is_some_and(|k| ALLOWED_ENV_VARS.contains(&k)))
}

/// Read `reader` to the end and return the last `max_bytes` bytes as text.
async fn read_tail(mut reader: impl AsyncRead + Unpin, max_bytes: usize) -> String {
    let mut tail = Vec::new();
    let mut buf = [0u8; 8192];
    while let Ok(n) = reader.read(&mut buf).await {
        if n == 0 {
            break;
        }
        tail.extend_from_slice(&buf[..n]);
        if tail.len() > max_bytes {
            tail.drain(..tail.len() - max_bytes);
        }
    }
    String::from_utf8_lossy(&tail).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect()
    }

    #[test]
    fn allowed_env_drops_kellnr_settings_and_unknown_vars() {
        let env = vars(&[
            ("PATH", "/usr/bin"),
            ("CARGO_HOME", "/cargo"),
            ("RUSTUP_TOOLCHAIN", "stable"),
            ("KELLNR_POSTGRESQL__PWD", "secret"),
            ("KELLNR_SETUP__ADMIN_TOKEN", "token"),
            ("AWS_SECRET_ACCESS_KEY", "aws"),
        ]);

        let allowed: Vec<_> = allowed_env(env.into_iter()).collect();

        assert_eq!(
            allowed,
            vars(&[
                ("PATH", "/usr/bin"),
                ("CARGO_HOME", "/cargo"),
                ("RUSTUP_TOOLCHAIN", "stable"),
            ])
        );
    }

    #[tokio::test]
    async fn read_tail_keeps_only_the_end_of_the_output() {
        let output = read_tail(&b"0123456789"[..], 4).await;

        assert_eq!(output, "6789");
    }

    /// Shell wrapper that records its environment and arguments to `out`
    /// instead of running the build.
    #[cfg(unix)]
    fn recording_wrapper(out: &Path) -> Vec<String> {
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "env > \"$0\"; echo \"args: $*\" >> \"$0\"".to_string(),
            out.display().to_string(),
        ]
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn build_runs_wrapper_with_kellnr_binary_and_cleared_env() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let builder = DocBuilder::new(
            PathBuf::from("/opt/kellnr/kellnr"),
            recording_wrapper(&out),
            Some("https://proxy/index/".to_string()),
        );

        builder.build(Path::new("/tmp/my-crate")).await.unwrap();

        let recorded = std::fs::read_to_string(&out).unwrap();
        assert!(recorded.contains(
            "args: /opt/kellnr/kellnr doc-build /tmp/my-crate --cratesio-index https://proxy/index/"
        ));
        for line in recorded.lines().filter(|l| !l.starts_with("args: ")) {
            let key = line.split('=').next().unwrap();
            // The shell itself sets a few variables like PWD or SHLVL.
            assert!(
                ALLOWED_ENV_VARS.contains(&key) || ["PWD", "SHLVL", "_", "OLDPWD"].contains(&key),
                "unexpected variable {key} passed to the docs build"
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn build_fails_with_output_of_failed_process() {
        let builder = DocBuilder::new(
            PathBuf::from("unused"),
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo 'error: could not compile' >&2; exit 101".to_string(),
            ],
            None,
        );

        let err = builder.build(Path::new("/tmp/my-crate")).await.unwrap_err();

        assert!(err.to_string().contains("error: could not compile"));
    }

    #[tokio::test]
    async fn build_fails_when_program_does_not_exist() {
        let builder = DocBuilder::new(PathBuf::from("/does/not/exist/kellnr"), vec![], None);

        let err = builder.build(Path::new("/tmp/my-crate")).await.unwrap_err();

        assert!(err.to_string().contains("failed to start docs build process"));
    }
}
