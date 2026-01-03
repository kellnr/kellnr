use std::borrow::Cow;
use std::collections::HashMap;

use testcontainers::Image;
use testcontainers::core::WaitFor;

const NAME: &str = "postgres";
const TAG: &str = "17-alpine";
const POSTGRES_PASSWORD: &str = "admin";
const POSTGRES_USER: &str = "admin";
const POSTGRES_DB: &str = "kellnr";

#[derive(Debug, Clone)]
pub struct Postgres {
    env_vars: HashMap<String, String>,
}

impl Postgres {
    pub const PG_PORT: u16 = 5432;
}

impl Default for Postgres {
    fn default() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("POSTGRES_DB".to_owned(), POSTGRES_DB.to_owned());
        env_vars.insert("POSTGRES_PASSWORD".to_owned(), POSTGRES_PASSWORD.to_owned());
        env_vars.insert("POSTGRES_USER".to_owned(), POSTGRES_USER.to_owned());
        Self { env_vars }
    }
}

impl Image for Postgres {
    fn name(&self) -> &str {
        NAME
    }

    fn tag(&self) -> &str {
        TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![
            WaitFor::message_on_stderr("database system is ready to accept connections"),
            WaitFor::seconds(1),
        ]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        self.env_vars.clone()
    }
}
