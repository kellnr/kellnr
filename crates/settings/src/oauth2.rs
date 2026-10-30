use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "profile".to_string(),
        "email".to_string(),
    ]
}

fn default_button_text() -> String {
    "Log in with SSO".to_string()
}

/// `OAuth2`/`OpenID` Connect authentication configuration
#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "oauth2")]
#[allow(clippy::struct_excessive_bools)]
pub struct OAuth2 {
    /// Enable `OAuth2`/OIDC authentication
    pub enabled: bool,

    /// OIDC issuer URL (discovery URL, e.g., `<https://authentik.example.com/application/o/kellnr/>`)
    pub issuer_url: Option<String>,

    /// `OAuth2` client ID
    pub client_id: Option<String>,

    /// `OAuth2` client secret (prefer setting via `KELLNR_OAUTH2__CLIENT_SECRET` env var)
    #[serde(skip_serializing)]
    #[configurable(secret)]
    pub client_secret: Option<String>,

    /// Additional audiences accepted in the ID token besides the client ID
    #[configurable(env_list)]
    #[arg(value_delimiter = ',')]
    pub additional_audiences: Vec<String>,

    /// `OAuth2` scopes to request (default: `["openid", "profile", "email"]`)
    #[configurable(env_list)]
    #[arg(value_delimiter = ',')]
    pub scopes: Vec<String>,

    /// Automatically create local user accounts for new `OAuth2` users
    pub auto_provision_users: bool,

    /// Claim name to check for admin group membership (e.g., "groups"),
    /// requires `admin_group_value`
    pub admin_group_claim: Option<String>,

    /// Value in the admin group claim that grants admin privileges (e.g., "kellnr-admins")
    pub admin_group_value: Option<String>,

    /// Claim name to check for read-only group membership (e.g., "groups"),
    /// requires `read_only_group_value`
    pub read_only_group_claim: Option<String>,

    /// Value in the read-only group claim that grants read-only access (e.g., "kellnr-readonly")
    pub read_only_group_value: Option<String>,

    /// Text displayed on the `OAuth2` login button
    pub button_text: String,

    /// Enforce SSO-only login, disabling local password login/change/reset and user creation (requires `enabled`)
    pub enforced: bool,

    /// Skip the login page and redirect straight to the SSO provider (requires `enforced`)
    pub auto_redirect: bool,
}

impl Default for OAuth2 {
    fn default() -> Self {
        Self {
            enabled: false,
            issuer_url: None,
            client_id: None,
            client_secret: None,
            additional_audiences: Vec::new(),
            scopes: default_scopes(),
            auto_provision_users: true,
            admin_group_claim: None,
            admin_group_value: None,
            read_only_group_claim: None,
            read_only_group_value: None,
            button_text: default_button_text(),
            enforced: false,
            auto_redirect: false,
        }
    }
}

impl OAuth2 {
    /// Validate the `OAuth2` configuration
    /// Returns an error message if the configuration is invalid
    pub fn validate(&self) -> Result<(), String> {
        if self.enforced && !self.enabled {
            return Err("OAuth2 enforced is set but OAuth2 is not enabled; \
                this would disable all login. Set oauth2.enabled = true"
                .to_string());
        }

        if self.auto_redirect && !self.enforced {
            return Err("OAuth2 auto_redirect is set but OAuth2 is not enforced; \
                set oauth2.enforced = true to redirect straight to SSO"
                .to_string());
        }

        if !self.enabled {
            return Ok(());
        }

        if is_unset(self.issuer_url.as_ref()) {
            return Err("OAuth2 is enabled but issuer_url is not set".to_string());
        }

        if is_unset(self.client_id.as_ref()) {
            return Err("OAuth2 is enabled but client_id is not set".to_string());
        }

        // client_secret may be optional for public clients using PKCE,
        // but we require it for confidential clients (default use case).
        // An empty secret is accepted, as some providers ignore it for
        // public clients.
        if self.client_secret.is_none() {
            return Err("OAuth2 is enabled but client_secret is not set. \
                Set it via KELLNR_OAUTH2__CLIENT_SECRET environment variable"
                .to_string());
        }

        if self.scopes.is_empty() {
            return Err("OAuth2 scopes cannot be empty".to_string());
        }

        if !self.scopes.contains(&"openid".to_string()) {
            return Err("OAuth2 scopes must contain 'openid' for OIDC".to_string());
        }

        Ok(())
    }

    /// Configuration that is accepted but most likely a mistake, for
    /// example a group claim without its value, which grants nothing.
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if !self.enabled {
            return warnings;
        }
        if self.admin_group_claim.is_some() != self.admin_group_value.is_some() {
            warnings.push(
                "OAuth2 admin_group_claim and admin_group_value must be set together, \
                 admin status is not synced from the provider"
                    .to_string(),
            );
        }
        if self.read_only_group_claim.is_some() != self.read_only_group_value.is_some() {
            warnings.push(
                "OAuth2 read_only_group_claim and read_only_group_value must be set \
                 together, read-only status is not synced from the provider"
                    .to_string(),
            );
        }
        warnings
    }
}

/// Whether a required setting is missing or empty.
fn is_unset(value: Option<&String>) -> bool {
    value.is_none_or(|v| v.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_oauth2() {
        let oauth2 = OAuth2::default();
        assert!(!oauth2.enabled);
        assert!(oauth2.issuer_url.is_none());
        assert!(oauth2.client_id.is_none());
        assert!(oauth2.client_secret.is_none());
        assert!(oauth2.additional_audiences.is_empty());
        assert_eq!(oauth2.scopes, vec!["openid", "profile", "email"]);
        assert!(oauth2.auto_provision_users);
        assert_eq!(oauth2.button_text, "Log in with SSO");
        assert!(!oauth2.enforced);
        assert!(!oauth2.auto_redirect);
    }

    #[test]
    fn test_validate_disabled() {
        let oauth2 = OAuth2::default();
        assert!(oauth2.validate().is_ok());
    }

    #[test]
    fn test_validate_enabled_missing_issuer() {
        let oauth2 = OAuth2 {
            enabled: true,
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("issuer_url"));
    }

    #[test]
    fn test_validate_enabled_missing_client_id() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("client_id"));
    }

    #[test]
    fn test_validate_enabled_missing_client_secret() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("client_secret"));
    }

    #[test]
    fn test_validate_enabled_valid() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            ..Default::default()
        };
        assert!(oauth2.validate().is_ok());
    }

    fn enabled() -> OAuth2 {
        OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_empty_required_values() {
        for oauth2 in [
            OAuth2 {
                issuer_url: Some(String::new()),
                ..enabled()
            },
            OAuth2 {
                client_id: Some(" ".to_string()),
                ..enabled()
            },
        ] {
            assert!(oauth2.validate().unwrap_err().contains("not set"));
        }
    }

    #[test]
    fn test_validate_accepts_empty_client_secret() {
        let oauth2 = OAuth2 {
            client_secret: Some(String::new()),
            ..enabled()
        };
        assert!(oauth2.validate().is_ok());
    }

    #[test]
    fn test_warnings_for_half_configured_group_claims() {
        for oauth2 in [
            OAuth2 {
                admin_group_claim: Some("groups".to_string()),
                ..enabled()
            },
            OAuth2 {
                admin_group_value: Some("admins".to_string()),
                ..enabled()
            },
            OAuth2 {
                read_only_group_claim: Some("groups".to_string()),
                ..enabled()
            },
            OAuth2 {
                read_only_group_value: Some("readers".to_string()),
                ..enabled()
            },
        ] {
            assert!(oauth2.validate().is_ok());
            let warnings = oauth2.warnings();
            assert_eq!(1, warnings.len());
            assert!(warnings[0].contains("set together"));
        }
    }

    #[test]
    fn test_validate_complete_group_claims() {
        let oauth2 = OAuth2 {
            admin_group_claim: Some("groups".to_string()),
            admin_group_value: Some("admins".to_string()),
            read_only_group_claim: Some("groups".to_string()),
            read_only_group_value: Some("readers".to_string()),
            ..enabled()
        };
        assert!(oauth2.validate().is_ok());
        assert!(oauth2.warnings().is_empty());
    }

    #[test]
    fn test_validate_empty_scopes() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            scopes: vec![],
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_validate_enforced_without_enabled() {
        let oauth2 = OAuth2 {
            enforced: true,
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("enforced"));
    }

    #[test]
    fn test_validate_enforced_with_valid_enabled() {
        let oauth2 = OAuth2 {
            enabled: true,
            enforced: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            ..Default::default()
        };
        assert!(oauth2.validate().is_ok());
    }

    #[test]
    fn test_validate_auto_redirect_without_enforced() {
        let oauth2 = OAuth2 {
            enabled: true,
            auto_redirect: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("auto_redirect"));
    }

    #[test]
    fn test_validate_auto_redirect_with_enforced() {
        let oauth2 = OAuth2 {
            enabled: true,
            enforced: true,
            auto_redirect: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            ..Default::default()
        };
        assert!(oauth2.validate().is_ok());
    }

    #[test]
    fn test_validate_missing_openid_scope() {
        let oauth2 = OAuth2 {
            enabled: true,
            issuer_url: Some("https://example.com".to_string()),
            client_id: Some("client-id".to_string()),
            client_secret: Some("client-secret".to_string()),
            scopes: vec!["profile".to_string(), "email".to_string()],
            ..Default::default()
        };
        let result = oauth2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("openid"));
    }
}
