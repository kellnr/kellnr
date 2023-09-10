use std::collections::HashMap;
use testcontainers::core::WaitFor;
use testcontainers::Image;

const NAME: &str = "postgres";
const TAG: &str = "14.3-alpine";
const POSTGRES_PASSWORD: &str = "admin";
const POSTGRES_USER: &str = "admin";
const POSTGRES_DB: &str = "kellnr";

#[derive(Debug)]
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
    type Args = ();

    fn name(&self) -> String {
        NAME.to_owned()
    }

    fn tag(&self) -> String {
        TAG.to_owned()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        )]
    }

    fn env_vars(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(self.env_vars.iter())
    }
}
