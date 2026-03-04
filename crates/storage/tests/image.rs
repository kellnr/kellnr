use std::borrow::Cow;
use std::collections::HashMap;

use testcontainers::Image;
use testcontainers::core::{ContainerPort, WaitFor};

const NAME: &str = "rustfs/rustfs";
const TAG: &str = "1.0.0-alpha.85-glibc";
const RUSTFS_ACCESS_KEY: &str = "rustfsadmin";
const RUSTFS_SECRET_KEY: &str = "rustfsadmin";

#[derive(Debug, Clone)]
pub struct RustFs {
    env_vars: HashMap<String, String>,
}

impl RustFs {
    pub const PORT: u16 = 9000;
    pub const CONTAINER_PORT: ContainerPort = ContainerPort::Tcp(Self::PORT);
}

impl Default for RustFs {
    fn default() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("RUSTFS_ACCESS_KEY".to_owned(), RUSTFS_ACCESS_KEY.to_owned());
        env_vars.insert("RUSTFS_SECRET_KEY".to_owned(), RUSTFS_SECRET_KEY.to_owned());
        env_vars.insert("RUSTFS_VOLUMES".to_owned(), "/data".to_owned());
        Self { env_vars }
    }
}

impl Image for RustFs {
    fn name(&self) -> &str {
        NAME
    }

    fn tag(&self) -> &str {
        TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("Starting:")]
    }

    fn entrypoint(&self) -> Option<&str> {
        // Override entrypoint to create the bucket directory before starting
        Some("sh")
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<Cow<'_, str>>> {
        vec![
            "-c",
            concat!(
                "mkdir -p /data/kellnr-crates && ",
                "echo 'Starting: rustfs /data' && ",
                "exec /usr/bin/rustfs ",
                "--access-key rustfsadmin ",
                "--secret-key rustfsadmin ",
                "/data"
            ),
        ]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[Self::CONTAINER_PORT]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        self.env_vars.clone()
    }
}
