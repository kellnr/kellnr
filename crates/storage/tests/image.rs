use std::borrow::Cow;
use std::collections::HashMap;
use testcontainers::Image;
use testcontainers::core::{ContainerPort, WaitFor};

const NAME: &str = "minio/minio";
const TAG: &str = "latest";
const MINIO_ROOT_USER: &str = "minioadmin";
const MINIO_ROOT_PASSWORD: &str = "minioadmin";
const MINIO_CONSOLE_ADDRESS: &str = ":9001";

#[derive(Debug, Clone)]
pub struct Minio {
    env_vars: HashMap<String, String>,
}

impl Minio {
    pub const PORT: u16 = 9000;
    pub const CONTAINER_PORT: ContainerPort = ContainerPort::Tcp(Self::PORT);
}

impl Default for Minio {
    fn default() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("MINIO_ROOT_USER".to_owned(), MINIO_ROOT_USER.to_owned());
        env_vars.insert(
            "MINIO_ROOT_PASSWORD".to_owned(),
            MINIO_ROOT_PASSWORD.to_owned(),
        );
        env_vars.insert(
            "MINIO_CONSOLE_ADDRESS".to_owned(),
            MINIO_CONSOLE_ADDRESS.to_owned(),
        );
        Self { env_vars }
    }
}

impl Image for Minio {
    fn name(&self) -> &str {
        NAME
    }

    fn tag(&self) -> &str {
        TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stderr("API:")]
    }

    fn entrypoint(&self) -> Option<&str> {
        Some("sh")
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<Cow<'_, str>>> {
        vec![
            "-c",
            "mkdir -p /data/kellnr-crates && /usr/bin/minio server /data",
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
