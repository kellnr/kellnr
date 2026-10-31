use std::fmt;

/// The secret of an auth token or a session as the client sends it. Only its
/// hash is stored. It has no `Display` and its `Debug` hides the secret, so it
/// cannot end up in logs by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct PlainToken(String);

impl PlainToken {
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    pub fn into_secret(self) -> String {
        self.0
    }
}

impl From<String> for PlainToken {
    fn from(token: String) -> Self {
        Self(token)
    }
}

impl From<&str> for PlainToken {
    fn from(token: &str) -> Self {
        Self(token.to_owned())
    }
}

impl fmt::Debug for PlainToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PlainToken(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_secret() {
        let token = PlainToken::from("s3cr3t");
        let debug = format!("{token:?}");
        assert_eq!("PlainToken(<redacted>)", debug);
        assert!(!debug.contains("s3cr3t"));
        assert_eq!("s3cr3t", token.expose_secret());
    }
}
