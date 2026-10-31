use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use kellnr_docs::docs_error::DocsError;
use kellnr_settings::cli::{DOC_BUILD_CRATESIO_INDEX_ARG, DOC_BUILD_SUBCOMMAND};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tracing::warn;

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

/// Prefix of cargo's own configuration variables, e.g. `CARGO_HOME`,
/// `CARGO_HTTP_CAINFO` or `CARGO_REGISTRIES_<NAME>_TOKEN`, which deployments
/// use to let the docs build authenticate against Kellnr's index. These are
/// visible to the crate's build scripts, so such a token should belong to a
/// dedicated read-only user.
const CARGO_ENV_PREFIX: &str = "CARGO_";

/// Keep at most this many bytes of the build output for error messages.
const OUTPUT_TAIL_BYTES: usize = 16 * 1024;
/// How long to wait for the build output after the build process exited.
/// Processes spawned by the build may keep the output pipe open.
const OUTPUT_GRACE_PERIOD: Duration = Duration::from_secs(5);

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
    timeout: Option<Duration>,
}

impl DocBuilder {
    pub fn new(program: PathBuf, wrapper: Vec<String>, cratesio_index: Option<String>) -> Self {
        Self {
            program,
            wrapper,
            cratesio_index,
            timeout: None,
        }
    }

    /// Kill builds that run longer than `timeout`, including all processes
    /// they started. `Duration::ZERO` disables the timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = (!timeout.is_zero()).then_some(timeout);
        self
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

        let status = match self.timeout {
            None => child.wait().await?,
            Some(timeout) => {
                let Ok(status) = tokio::time::timeout(timeout, child.wait()).await else {
                    kill_build(&mut child).await;
                    output.abort();
                    return Err(DocsError::BuildTimeout(timeout.as_secs()));
                };
                status?
            }
        };
        let output = match tokio::time::timeout(OUTPUT_GRACE_PERIOD, output).await {
            Ok(output) => output.unwrap_or_default(),
            Err(_) => String::from("<build output not available>"),
        };
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
        // Run the build in its own process group, so a timeout kills the
        // compiler and build scripts started by cargo as well.
        #[cfg(unix)]
        command.process_group(0);
        command
    }
}

/// Kill the build process and, on Unix, its whole process group.
async fn kill_build(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child
        .id()
        .and_then(|id| i32::try_from(id).ok())
        .and_then(rustix::process::Pid::from_raw)
        && let Err(e) = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL)
    {
        warn!("Failed to kill docs build process group: {e}");
    }
    if let Err(e) = child.kill().await {
        warn!("Failed to kill docs build process: {e}");
    }
}

fn allowed_env(
    vars: impl Iterator<Item = (OsString, OsString)>,
) -> impl Iterator<Item = (OsString, OsString)> {
    vars.filter(|(key, _)| key.to_str().is_some_and(is_allowed_env_var))
}

fn is_allowed_env_var(key: &str) -> bool {
    ALLOWED_ENV_VARS.contains(&key) || key.starts_with(CARGO_ENV_PREFIX)
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
            ("CARGO_REGISTRIES_KELLNR_TOKEN", "doc-token"),
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
                ("CARGO_REGISTRIES_KELLNR_TOKEN", "doc-token"),
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
                is_allowed_env_var(key) || ["PWD", "SHLVL", "_", "OLDPWD"].contains(&key),
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

    #[cfg(unix)]
    fn process_is_alive(pid: &str) -> bool {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .status()
            .unwrap()
            .success()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn build_timeout_kills_build_and_its_child_processes() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid");
        let builder = DocBuilder::new(
            PathBuf::from("unused"),
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 30 & echo $! > \"$0\"; wait".to_string(),
                pid_file.display().to_string(),
            ],
            None,
        )
        .with_timeout(Duration::from_millis(500));

        let started = std::time::Instant::now();
        let err = builder.build(Path::new("/tmp/my-crate")).await.unwrap_err();

        assert!(matches!(err, DocsError::BuildTimeout(_)));
        assert!(started.elapsed() < Duration::from_secs(10));
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        // The killed process may need a moment to be reaped.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while process_is_alive(pid.trim()) && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(!process_is_alive(pid.trim()));
    }

    #[test]
    fn zero_timeout_disables_the_timeout() {
        let builder =
            DocBuilder::new(PathBuf::from("unused"), vec![], None).with_timeout(Duration::ZERO);

        assert_eq!(builder.timeout, None);
    }

    #[tokio::test]
    async fn build_fails_when_program_does_not_exist() {
        let builder = DocBuilder::new(PathBuf::from("/does/not/exist/kellnr"), vec![], None);

        let err = builder.build(Path::new("/tmp/my-crate")).await.unwrap_err();

        assert!(
            err.to_string()
                .contains("failed to start docs build process")
        );
    }
}
